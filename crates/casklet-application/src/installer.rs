//! Installer orchestration. Correlation creates references; verified MSI ownership uses its provider.
use crate::projection::{ManifestProjection, ProjectionRepository};
use crate::resource::{ResourceRepository, ResourceStorage};
use crate::windows_inventory::{
    InstalledInventoryRepository, InstalledInventorySource, ObservedInstallation,
};
use crate::{CaskRepository, CaskService, api_error, check_version};
use casklet_contracts::{
    AddMode, AddResource, ApiError, CONTRACT_VERSION, CancelInstallerOperation,
    CloseInstallerOperation, ConfirmInstallerCandidate, GetInstallerOperation,
    InspectInstallerArtifact, InstallerArtifactView, InstallerCandidateView, InstallerInspection,
    InstallerOperationView, LaunchInstallerArtifact, ListInstallerArtifacts,
    ListInstallerOperations, RegisterInstallerArtifact,
};
use casklet_domain::CaskState;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerOutcome {
    Exited(i32),
    /// Windows launcher waited for the executable and its descendants.
    ProcessTreeExited(i32),
    TimedOut,
    Cancelled,
    ElevationDeclined,
}

pub trait InstallerSource {
    /// # Errors
    /// Inspects bytes and native Windows metadata without executing the artifact.
    fn inspect_installer(&self, path: &str) -> Result<InstallerInspection, String>;
    /// # Errors
    /// Launches only the selected, revalidated file; returns an uncertain outcome on process errors.
    fn run_installer(
        &self,
        artifact: &InstallerInspection,
        allow_elevation: bool,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<InstallerOutcome, String>;
}

pub trait InstallerRepository {
    /// # Errors
    /// Stores a referenced artifact linked to an existing resource.
    fn register_installer(&self, view: &InstallerArtifactView) -> Result<(), String>;
    /// # Errors
    /// Gets an artifact.
    fn get_installer(&self, id: &str) -> Result<Option<InstallerArtifactView>, String>;
    /// # Errors
    /// Lists artifacts for a Cask.
    fn list_installers(&self, cask_id: &str) -> Result<Vec<InstallerArtifactView>, String>;
    /// # Errors
    /// Journals intent and takes a mutation lock before external work.
    fn begin_installer(
        &self,
        id: &str,
        artifact_id: &str,
        cask_id: &str,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Heartbeats a running operation.
    fn installer_heartbeat(&self, id: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Requests cancellation.
    fn cancel_installer(&self, id: &str) -> Result<(), String>;
    /// # Errors
    /// Reads cancellation state.
    fn installer_cancelled(&self, id: &str) -> Result<bool, String>;
    /// # Errors
    /// Commits candidates and outcome after external work.
    fn finish_installer(
        &self,
        id: &str,
        state: &str,
        exit_code: Option<i32>,
        error_code: Option<&str>,
        candidates: &[InstallerCandidateView],
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Gets an operation and detects stale running work.
    fn get_installer_operation(&self, id: &str) -> Result<Option<InstallerOperationView>, String>;
    /// # Errors
    /// Lists operations for a Cask.
    fn list_installer_operations(
        &self,
        cask_id: &str,
    ) -> Result<Vec<InstallerOperationView>, String>;
    /// # Errors
    /// Closes uncertain work without granting ownership.
    fn close_installer_preserved(&self, id: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Records confirmation and optional install provenance atomically.
    fn confirm_installer_candidate(
        &self,
        id: &str,
        installation_id: &str,
        evidence_sha256: &str,
        now: i64,
        installed_for_cask: bool,
    ) -> Result<(), String>;
}

fn clock(correlation_id: &str) -> Result<i64, ApiError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
            .as_millis(),
    )
    .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))
}
fn fail(code: &str, message: impl Into<String>, correlation_id: &str) -> ApiError {
    api_error(code, message.into(), correlation_id)
}

fn candidate(
    item: &ObservedInstallation,
    inspection: &InstallerInspection,
) -> InstallerCandidateView {
    let product_code_match = inspection.product_code.as_ref().is_some_and(|code| {
        item.source_key
            .to_ascii_lowercase()
            .ends_with(&code.to_ascii_lowercase())
    });
    let name_match = inspection
        .product_name
        .as_ref()
        .is_some_and(|name| name.eq_ignore_ascii_case(&item.name));
    let publisher_match = inspection
        .publisher
        .as_ref()
        .zip(item.publisher.as_ref())
        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
    let mut evidence = vec!["New Windows uninstall registration after launch".into()];
    if product_code_match {
        evidence.push("MSI ProductCode matches registration key".into());
    }
    if name_match {
        evidence.push("Product name matches".into());
    }
    if publisher_match {
        evidence.push("Publisher matches".into());
    }
    let confidence = if product_code_match {
        "HIGH"
    } else if name_match && publisher_match {
        "MEDIUM"
    } else {
        "LOW"
    };
    InstallerCandidateView {
        installation_id: item.installation_id.clone(),
        evidence_sha256: item.evidence_sha256.clone(),
        name: item.name.clone(),
        publisher: item.publisher.clone(),
        display_version: item.display_version.clone(),
        scope: item.scope.clone(),
        confidence: confidence.into(),
        evidence,
    }
}

impl<R, S> CaskService<R, S>
where
    R: CaskRepository
        + ResourceRepository
        + ProjectionRepository
        + InstalledInventoryRepository
        + InstallerRepository
        + crate::managed_msi::MsiRepository,
    S: ResourceStorage + ManifestProjection + InstalledInventorySource + InstallerSource,
{
    /// # Errors
    /// Copies and runs explicitly selected bytes, then resolves supported outcomes.
    pub fn install_application(
        &self,
        request: &casklet_contracts::InstallApplication,
        correlation_id: &str,
        started: &dyn Fn(&str),
    ) -> Result<InstallerOperationView, ApiError>
    where
        S: crate::managed_msi::MsiUninstaller,
    {
        check_version(request.version, correlation_id)?;
        if !request.accept_execution_risk {
            return Err(fail(
                "REVIEW_REQUIRED",
                "Choose Install to authorize this installer",
                correlation_id,
            ));
        }
        let artifact = self.register_installer_artifact(
            &RegisterInstallerArtifact {
                version: request.version,
                cask_id: request.cask_id.clone(),
                path: request.path.clone(),
                expected_sha256: request.expected_sha256.clone(),
            },
            correlation_id,
        )?;
        let launch = LaunchInstallerArtifact {
            version: request.version,
            artifact_id: artifact.id,
            expected_sha256: artifact.inspection.sha256,
            accept_execution_risk: true,
            // Elevation is explicit and limited to this installer.
            allow_elevation: request.allow_elevation,
        };
        let managed = artifact.inspection.format == "MSI"
            && artifact
                .inspection
                .product_code
                .as_deref()
                .and_then(crate::managed_msi::product_code)
                .is_some();
        let operation = if managed {
            self.launch_managed_msi_artifact(&launch, correlation_id, started)?
        } else {
            self.launch_installer_artifact(&launch, correlation_id, started)?
        };
        if operation.state == "CONFIRMED_ASSOCIATION" {
            return Ok(operation);
        }
        // Install and Associate Existing App have separate ownership transactions.
        if operation.state == "REVIEW_REQUIRED"
            && operation.exit_code == Some(0)
            && operation.candidates.len() == 1
            && matches!(
                operation.candidates[0].confidence.as_str(),
                "HIGH" | "MEDIUM"
            )
        {
            let claim = ConfirmInstallerCandidate {
                version: request.version,
                operation_id: operation.id.clone(),
                installation_id: operation.candidates[0].installation_id.clone(),
            };
            self.claim_managed_msi(&claim, correlation_id)?;
            return self.get_installer_operation(
                &GetInstallerOperation {
                    version: request.version,
                    id: operation.id,
                },
                correlation_id,
            );
        }
        Ok(operation)
    }
    /// # Errors
    /// Returns file or metadata inspection errors.
    pub fn inspect_installer_artifact(
        &self,
        request: &InspectInstallerArtifact,
        correlation_id: &str,
    ) -> Result<InstallerInspection, ApiError> {
        check_version(request.version, correlation_id)?;
        self.storage
            .inspect_installer(&request.path)
            .map_err(|e| fail("INSTALLER_INSPECTION", e, correlation_id))
    }
    /// # Errors
    /// Registers the reviewed file as a referenced resource and installer artifact.
    pub fn register_installer_artifact(
        &self,
        request: &RegisterInstallerArtifact,
        correlation_id: &str,
    ) -> Result<InstallerArtifactView, ApiError> {
        check_version(request.version, correlation_id)?;
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| fail("NOT_FOUND", "Cask not found", correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(fail("INVALID_STATE", "Cask is not active", correlation_id));
        }
        let inspection = self.inspect_installer_artifact(
            &InspectInstallerArtifact {
                version: CONTRACT_VERSION,
                path: request.path.clone(),
            },
            correlation_id,
        )?;
        if inspection.sha256 != request.expected_sha256 {
            return Err(fail(
                "REVIEW_CHANGED",
                "Installer bytes changed after review",
                correlation_id,
            ));
        }
        let resource = self.add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: request.cask_id.clone(),
                source_path: inspection.path.clone(),
                mode: AddMode::Copy,
                category_key: "applications".into(),
            },
            correlation_id,
            &|_, _| {},
        )?;
        let copy = self
            .storage
            .inspect_installer(&resource.path)
            .map_err(|e| fail("INSTALLER_INSPECTION", e, correlation_id))?;
        if copy.sha256 != inspection.sha256 {
            return Err(fail(
                "REVIEW_CHANGED",
                "Managed installer copy differs from the reviewed source",
                correlation_id,
            ));
        }
        let view=InstallerArtifactView{version:CONTRACT_VERSION,id:Uuid::new_v4().to_string(),cask_id:request.cask_id.clone(),resource_id:resource.id,source_path:inspection.path,inspection:copy,registered_at_ms:clock(correlation_id)?,warnings:vec!["Managed installer copy. Registration never launches it or grants installation ownership.".into()]};
        self.repository
            .register_installer(&view)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?;
        Ok(view)
    }
    /// # Errors
    /// Lists registered artifacts.
    pub fn list_installer_artifacts(
        &self,
        request: &ListInstallerArtifacts,
        correlation_id: &str,
    ) -> Result<Vec<InstallerArtifactView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list_installers(&request.cask_id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Launches reviewed bytes and automatically claims only a verified new MSI installation.
    pub fn launch_managed_msi_artifact(
        &self,
        request: &LaunchInstallerArtifact,
        correlation_id: &str,
        started: &dyn Fn(&str),
    ) -> Result<InstallerOperationView, ApiError>
    where
        S: crate::managed_msi::MsiUninstaller,
    {
        check_version(request.version, correlation_id)?;
        let artifact = self
            .repository
            .get_installer(&request.artifact_id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| fail("NOT_FOUND", "Installer not found", correlation_id))?;
        if artifact.inspection.format != "MSI"
            || artifact
                .inspection
                .product_code
                .as_deref()
                .and_then(crate::managed_msi::product_code)
                .is_none()
        {
            return Err(fail(
                "UNSUPPORTED_INSTALLER",
                "Managed installation requires a supported MSI package",
                correlation_id,
            ));
        }
        let operation = self.launch_installer_artifact(request, correlation_id, started)?;
        if operation.state == "REVIEW_REQUIRED"
            && operation.exit_code == Some(0)
            && operation.candidates.len() == 1
        {
            let claim = ConfirmInstallerCandidate {
                version: request.version,
                operation_id: operation.id.clone(),
                installation_id: operation.candidates[0].installation_id.clone(),
            };
            // The launch action explicitly requested management, but only verified baseline
            // evidence can grant it. Uncertain results stay reviewable with no ownership.
            if self.claim_managed_msi(&claim, correlation_id).is_ok() {
                return self.get_installer_operation(
                    &GetInstallerOperation {
                        version: request.version,
                        id: operation.id,
                    },
                    correlation_id,
                );
            }
        }
        Ok(operation)
    }
    /// # Errors
    /// Launches reviewed bytes and returns a durable review result without ownership.
    #[expect(
        clippy::too_many_lines,
        reason = "Each inventory and journal boundary remains explicit"
    )]
    pub fn launch_installer_artifact(
        &self,
        request: &LaunchInstallerArtifact,
        correlation_id: &str,
        started: &dyn Fn(&str),
    ) -> Result<InstallerOperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        if !request.accept_execution_risk {
            return Err(fail(
                "REVIEW_REQUIRED",
                "Explicit installer execution approval is required",
                correlation_id,
            ));
        }
        let artifact = self
            .repository
            .get_installer(&request.artifact_id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| fail("NOT_FOUND", "Installer artifact not found", correlation_id))?;
        let cask = self
            .repository
            .get(&artifact.cask_id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| fail("NOT_FOUND", "Cask not found", correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(fail("INVALID_STATE", "Cask is not active", correlation_id));
        }
        if request.expected_sha256 != artifact.inspection.sha256 {
            return Err(fail(
                "REVIEW_CHANGED",
                "Installer hash differs from registration",
                correlation_id,
            ));
        }
        let current = self
            .storage
            .inspect_installer(&artifact.inspection.path)
            .map_err(|e| fail("INSTALLER_INSPECTION", e, correlation_id))?;
        if current.sha256 != artifact.inspection.sha256
            || current.path != artifact.inspection.path
            || current.signature_status != artifact.inspection.signature_status
            || current.signer != artifact.inspection.signer
            || current.product_code != artifact.inspection.product_code
        {
            return Err(fail(
                "REVIEW_CHANGED",
                "Installer identity or signature changed",
                correlation_id,
            ));
        }
        let id = Uuid::new_v4().to_string();
        self.repository
            .begin_installer(&id, &artifact.id, &artifact.cask_id, clock(correlation_id)?)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?;
        if catch_unwind(AssertUnwindSafe(|| started(&id))).is_err() {
            let _ = self.repository.finish_installer(
                &id,
                "RECOVERY_REQUIRED",
                None,
                Some("PRESENTATION_INTERRUPTED"),
                &[],
                clock(correlation_id)?,
            );
            return Err(fail(
                "RECOVERY_REQUIRED",
                format!("Operation {id}: notification failed; installer was not launched"),
                correlation_id,
            ));
        }
        let before = self.storage.scan_installed().map_err(|e| {
            let _ = self.repository.finish_installer(
                &id,
                "RECOVERY_REQUIRED",
                None,
                Some("PRE_INVENTORY_UNVERIFIED"),
                &[],
                clock(correlation_id).unwrap_or(0),
            );
            fail(
                "RECOVERY_REQUIRED",
                format!("Operation {id}: inventory unavailable: {e}"),
                correlation_id,
            )
        })?;
        if current.format == "MSI"
            && let Some(code) = current
                .product_code
                .as_deref()
                .and_then(crate::managed_msi::product_code)
        {
            let absent = !before.iter().any(|item| {
                item.source_key
                    .rsplit('\\')
                    .next()
                    .is_some_and(|key| key.eq_ignore_ascii_case(&code))
            });
            if let Err(reason) = self.repository.msi_baseline(&id, &code, absent) {
                let _ = self.repository.finish_installer(
                    &id,
                    "RECOVERY_REQUIRED",
                    None,
                    Some("BASELINE_NOT_RECORDED"),
                    &[],
                    clock(correlation_id)?,
                );
                return Err(fail("RECOVERY_REQUIRED", reason, correlation_id));
            }
        }
        let run = catch_unwind(AssertUnwindSafe(|| {
            self.storage.run_installer(
                &current,
                request.allow_elevation,
                &|| self.repository.installer_cancelled(&id).unwrap_or(true),
                &|| {
                    let _ = self
                        .repository
                        .installer_heartbeat(&id, clock(correlation_id).unwrap_or(0));
                },
            )
        }))
        .unwrap_or_else(|_| Err("Installer adapter interrupted".into()));
        let after = self.storage.scan_installed();
        let candidates = after.as_ref().map_or_else(
            |_| vec![],
            |items| {
                items
                    .iter()
                    .filter(|item| {
                        !before
                            .iter()
                            .any(|old| old.installation_id == item.installation_id)
                    })
                    .map(|item| candidate(item, &current))
                    .collect()
            },
        );
        let (state, exit_code, error_code) = match (&run, &after) {
            (Ok(InstallerOutcome::ProcessTreeExited(0)), Ok(_)) => {
                ("REVIEW_REQUIRED", Some(0), None)
            }
            (Ok(InstallerOutcome::Exited(0)), Ok(_)) if current.format == "MSI" => {
                ("REVIEW_REQUIRED", Some(0), None)
            }
            (Ok(InstallerOutcome::Exited(0)), Ok(_)) => (
                "RECOVERY_REQUIRED",
                Some(0),
                Some("CHILD_PROCESS_UNVERIFIED"),
            ),
            (
                Ok(
                    InstallerOutcome::Exited(3010 | 1641)
                    | InstallerOutcome::ProcessTreeExited(3010 | 1641),
                ),
                _,
            ) => (
                "RECOVERY_REQUIRED",
                run.as_ref().ok().and_then(|v| {
                    if let InstallerOutcome::Exited(code)
                    | InstallerOutcome::ProcessTreeExited(code) = v
                    {
                        Some(*code)
                    } else {
                        None
                    }
                }),
                Some("REBOOT_REQUIRED"),
            ),
            (Ok(InstallerOutcome::Exited(code) | InstallerOutcome::ProcessTreeExited(code)), _) => {
                (
                    "RECOVERY_REQUIRED",
                    Some(*code),
                    Some("INSTALLER_EXIT_UNCERTAIN"),
                )
            }
            (Ok(InstallerOutcome::TimedOut), _) => ("RECOVERY_REQUIRED", None, Some("TIMED_OUT")),
            (Ok(InstallerOutcome::Cancelled), _) => ("RECOVERY_REQUIRED", None, Some("CANCELLED")),
            (Ok(InstallerOutcome::ElevationDeclined), _) => {
                ("RECOVERY_REQUIRED", None, Some("ELEVATION_DECLINED"))
            }
            (Err(_), _) => ("RECOVERY_REQUIRED", None, Some("LAUNCH_UNCERTAIN")),
        };
        let (state, error_code) = if after.is_err() {
            ("RECOVERY_REQUIRED", Some("POST_INVENTORY_UNVERIFIED"))
        } else if self.repository.installer_cancelled(&id).unwrap_or(true) {
            ("RECOVERY_REQUIRED", Some("CANCELLED_AFTER_LAUNCH"))
        } else {
            (state, error_code)
        };
        self.repository
            .finish_installer(
                &id,
                state,
                exit_code,
                error_code,
                &candidates,
                clock(correlation_id)?,
            )
            .map_err(|e| {
                fail(
                    "RECOVERY_REQUIRED",
                    format!("Operation {id}: {e}"),
                    correlation_id,
                )
            })?;
        self.repository
            .get_installer_operation(&id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| fail("NOT_FOUND", "Operation missing", correlation_id))
    }
    /// # Errors
    /// Returns a durable operation.
    pub fn get_installer_operation(
        &self,
        request: &GetInstallerOperation,
        correlation_id: &str,
    ) -> Result<InstallerOperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .get_installer_operation(&request.id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| fail("NOT_FOUND", "Operation not found", correlation_id))
    }
    /// # Errors
    /// Lists durable operations.
    pub fn list_installer_operations(
        &self,
        request: &ListInstallerOperations,
        correlation_id: &str,
    ) -> Result<Vec<InstallerOperationView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list_installer_operations(&request.cask_id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Requests cooperative process cancellation.
    pub fn cancel_installer_operation(
        &self,
        request: &CancelInstallerOperation,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .cancel_installer(&request.id)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Records an explicit association with current registry evidence, never ownership.
    pub fn confirm_installer_candidate(
        &self,
        request: &ConfirmInstallerCandidate,
        correlation_id: &str,
    ) -> Result<InstallerOperationView, ApiError> {
        self.confirm_candidate(request, correlation_id, false)
    }
    fn confirm_candidate(
        &self,
        request: &ConfirmInstallerCandidate,
        correlation_id: &str,
        installed_for_cask: bool,
    ) -> Result<InstallerOperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        let operation = self.get_installer_operation(
            &GetInstallerOperation {
                version: CONTRACT_VERSION,
                id: request.operation_id.clone(),
            },
            correlation_id,
        )?;
        if !matches!(
            operation.state.as_str(),
            "REVIEW_REQUIRED" | "RECOVERY_REQUIRED"
        ) {
            return Err(fail(
                "INVALID_STATE",
                "Operation is not awaiting review",
                correlation_id,
            ));
        }
        let candidate = operation
            .candidates
            .iter()
            .find(|item| item.installation_id == request.installation_id)
            .ok_or_else(|| {
                fail(
                    "NOT_FOUND",
                    "Candidate is not part of this operation",
                    correlation_id,
                )
            })?;
        let observed = self
            .storage
            .scan_installed()
            .map_err(|e| fail("INVENTORY", e, correlation_id))?
            .into_iter()
            .find(|item| {
                item.installation_id == request.installation_id
                    && item.evidence_sha256 == candidate.evidence_sha256
            })
            .ok_or_else(|| {
                fail(
                    "EVIDENCE_CHANGED",
                    "Candidate registration changed or disappeared",
                    correlation_id,
                )
            })?;
        self.repository
            .associate_installed(&operation.cask_id, &observed, clock(correlation_id)?)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?;
        self.repository
            .confirm_installer_candidate(
                &operation.id,
                &observed.installation_id,
                &observed.evidence_sha256,
                clock(correlation_id)?,
                installed_for_cask,
            )
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))?;
        self.get_installer_operation(
            &GetInstallerOperation {
                version: CONTRACT_VERSION,
                id: operation.id,
            },
            correlation_id,
        )
    }
    /// # Errors
    /// Closes an uncertain launch while preserving external changes.
    pub fn close_installer_operation(
        &self,
        request: &CloseInstallerOperation,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .close_installer_preserved(&request.id, clock(correlation_id)?)
            .map_err(|e| fail("PERSISTENCE", e, correlation_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn msi_product_code_is_stronger_evidence_than_name_and_never_ownership() {
        let product_code = "{12345678-1234-1234-1234-123456789abc}";
        let inspection = InstallerInspection {
            version: CONTRACT_VERSION,
            path: "C:\\setup.msi".into(),
            format: "MSI".into(),
            sha256: "hash".into(),
            size_bytes: 1,
            signature_status: "VALID".into(),
            signature_detail: Some("Signature valid".into()),
            signer: Some("CN=Publisher".into()),
            product_name: Some("Different name".into()),
            publisher: None,
            product_version: None,
            product_code: Some(product_code.into()),
            warnings: vec![],
        };
        let observed = ObservedInstallation {
            application_id: "app".into(),
            installation_id: "instance".into(),
            source_key: format!("HKLM64\\Uninstall\\{product_code}"),
            scope: "SYSTEM".into(),
            name: "Actual name".into(),
            publisher: None,
            display_version: None,
            install_location: None,
            evidence_json: "{}".into(),
            evidence_sha256: "evidence".into(),
        };
        let view = candidate(&observed, &inspection);
        assert_eq!(view.confidence, "HIGH");
        assert!(
            view.evidence
                .iter()
                .any(|item| item.contains("ProductCode"))
        );
        assert_eq!(view.installation_id, "instance");
    }
}
