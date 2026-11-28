//! `WinGet` lifecycle orchestration. Exit status alone never establishes installation ownership.
use crate::windows_inventory::{InstalledInventorySource, ObservedInstallation};
use crate::{CaskRepository, CaskService, api_error, check_version};
#[cfg(feature = "reviewed-removal")]
use casklet_contracts::ExecuteWingetUninstallPlan;
use casklet_contracts::{
    ApiError, ApproveWingetUninstallPlan, CONTRACT_VERSION, CancelWingetOperation,
    CloseWingetOperation, CreateWingetUninstallPlan, GetWingetOperation, InstallWingetPackage,
    ListWingetInstallations, ReviewWingetPackage, SearchWinget, WingetCapabilities,
    WingetInstallationView, WingetOperationView, WingetPackageReview, WingetSearchResult,
    WingetUninstallPlanView,
};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    pub id: String,
    pub version: String,
}

pub trait WingetProvider {
    fn capabilities(&self) -> WingetCapabilities;
    /// # Errors
    /// Returns a bounded, sanitized search result.
    fn search(&self, query: &str) -> Result<String, String>;
    /// # Errors
    /// Returns bounded package details and the verified provider binary hash.
    fn show(&self, id: &str, version: &str) -> Result<(String, String), String>;
    /// # Errors
    /// Returns a complete, structured installed-package snapshot.
    fn installed_packages(&self) -> Result<Vec<InstalledPackage>, String>;
    /// # Errors
    /// Reports timeout, cancellation, nonzero exit, or process failures.
    fn install(
        &self,
        id: &str,
        version: &str,
        accept_agreements: bool,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<(), String>;
    /// # Errors
    /// Reports timeout, cancellation, nonzero exit, or process failures.
    fn uninstall(
        &self,
        id: &str,
        version: &str,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct WingetOwnedRecord {
    pub view: WingetInstallationView,
    pub registry_evidence_sha256: String,
    pub provider_sha256: String,
    pub revision: i64,
}

pub trait WingetRepository {
    /// # Errors
    /// Records durable intent and claims a Cask lock before external work.
    fn begin_winget(
        &self,
        op_id: &str,
        cask_id: &str,
        action: &str,
        id: &str,
        version: &str,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Updates step and heartbeat without holding a transaction across provider work.
    fn winget_step(&self, op_id: &str, step: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Atomically marks uncertain work, retaining the lock.
    fn winget_uncertain(&self, op_id: &str, code: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Atomically records a verified new instance and closes the operation.
    #[expect(
        clippy::too_many_arguments,
        reason = "Registration binds exact operation, package, provider and instance evidence"
    )]
    fn finish_winget_install(
        &self,
        op_id: &str,
        cask_id: &str,
        id: &str,
        version: &str,
        provider_hash: &str,
        observed: &ObservedInstallation,
        now: i64,
    ) -> Result<WingetInstallationView, String>;
    /// # Errors
    /// Returns an exact owned installation.
    fn get_winget_installation(&self, id: &str) -> Result<Option<WingetOwnedRecord>, String>;
    /// # Errors
    /// Returns Casklet-created `WinGet` installations.
    fn list_winget_installations(
        &self,
        cask_id: &str,
    ) -> Result<Vec<WingetInstallationView>, String>;
    /// # Errors
    /// Returns current operation state.
    fn get_winget_operation(&self, id: &str) -> Result<Option<WingetOperationView>, String>;
    /// # Errors
    /// Requests cooperative cancellation.
    fn cancel_winget(&self, id: &str) -> Result<(), String>;
    /// # Errors
    /// Returns the cancellation flag.
    fn winget_cancelled(&self, id: &str) -> Result<bool, String>;
    /// # Errors
    /// Closes uncertain work without claiming a result.
    fn close_winget_preserved(&self, id: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Returns number of other active associations blocking uninstall.
    fn winget_blocking_associations(
        &self,
        installation_id: &str,
        registry_id: &str,
    ) -> Result<u64, String>;
    /// # Errors
    /// Stores an inspectable uninstall plan.
    fn save_winget_plan(
        &self,
        plan: &WingetUninstallPlanView,
        record: &WingetOwnedRecord,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Loads a previously generated plan.
    fn get_winget_plan(&self, id: &str) -> Result<Option<WingetUninstallPlanView>, String>;
    /// # Errors
    /// Approves only the exact current plan and installation revision.
    fn approve_winget_plan(&self, id: &str, hash: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Begins uninstall with the exact approved plan and a durable lock.
    fn begin_winget_uninstall(
        &self,
        plan_id: &str,
        hash: &str,
        op_id: &str,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Marks successful external uninstall after independent absence checks.
    fn finish_winget_uninstall(&self, plan_id: &str, op_id: &str, now: i64) -> Result<(), String>;
}

fn now(correlation_id: &str) -> Result<i64, ApiError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
            .as_millis(),
    )
    .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))
}
fn valid_id(value: &str) -> bool {
    value.len() <= 120
        && !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}
fn valid_version(value: &str) -> bool {
    value.len() <= 80
        && !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn matches_package(packages: &[InstalledPackage], id: &str, version: &str) -> usize {
    packages
        .iter()
        .filter(|p| p.id == id && p.version == version)
        .count()
}
fn reviewed_name(details: &str, id: &str) -> Option<String> {
    let first = details.lines().next()?.trim();
    let suffix = format!("[{id}]");
    let name = first.strip_prefix("Found ")?.strip_suffix(&suffix)?.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        None
    } else {
        Some(name.into())
    }
}

impl<R: CaskRepository + WingetRepository, S: WingetProvider + InstalledInventorySource>
    CaskService<R, S>
{
    #[must_use]
    pub fn winget_capabilities(&self) -> WingetCapabilities {
        self.storage.capabilities()
    }
    /// # Errors
    /// Returns a provider error or invalid query.
    pub fn search_winget(
        &self,
        request: &SearchWinget,
        correlation_id: &str,
    ) -> Result<WingetSearchResult, ApiError> {
        check_version(request.version, correlation_id)?;
        if request.query.is_empty()
            || request.query.len() > 120
            || request.query.chars().any(char::is_control)
        {
            return Err(api_error(
                "INVALID_QUERY",
                "Search query is invalid".into(),
                correlation_id,
            ));
        }
        Ok(WingetSearchResult {
            version: CONTRACT_VERSION,
            sanitized_text: self
                .storage
                .search(&request.query)
                .map_err(|e| api_error("PROVIDER", e, correlation_id))?,
        })
    }
    /// # Errors
    /// Returns an invalid ID/version or provider error.
    pub fn review_winget_package(
        &self,
        request: &ReviewWingetPackage,
        correlation_id: &str,
    ) -> Result<WingetPackageReview, ApiError> {
        check_version(request.version, correlation_id)?;
        if !valid_id(&request.package_id) || !valid_version(&request.package_version) {
            return Err(api_error(
                "INVALID_PACKAGE",
                "Package ID or version is invalid".into(),
                correlation_id,
            ));
        }
        let (details, provider_hash) = self
            .storage
            .show(&request.package_id, &request.package_version)
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        if reviewed_name(&details, &request.package_id).is_none() {
            return Err(api_error(
                "AMBIGUOUS_PACKAGE",
                "Provider did not return one exact package".into(),
                correlation_id,
            ));
        }
        let review_hash = digest(&format!(
            "{}\0{}\0{}\0{}",
            request.package_id, request.package_version, provider_hash, details
        ));
        Ok(WingetPackageReview {version:CONTRACT_VERSION,package_id:request.package_id.clone(),package_version:request.package_version.clone(),source:"winget".into(),details,review_hash,warnings:vec!["Installer may affect the whole user or machine. Review the package and license before continuing.".into()]})
    }
    /// # Errors
    /// Leaves uncertain external outcomes in a durable operation without ownership.
    #[expect(
        clippy::too_many_lines,
        reason = "Install keeps each external and durable boundary explicit"
    )]
    pub fn install_winget_package(
        &self,
        request: &InstallWingetPackage,
        correlation_id: &str,
        started: &dyn Fn(&str),
    ) -> Result<WingetInstallationView, ApiError> {
        check_version(request.version, correlation_id)?;
        if !request.accept_package_agreements {
            return Err(api_error(
                "AGREEMENTS_REQUIRED",
                "Package agreements must be accepted explicitly".into(),
                correlation_id,
            ));
        }
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.state != casklet_domain::CaskState::Active {
            return Err(api_error(
                "INVALID_STATE",
                "Cask is not active".into(),
                correlation_id,
            ));
        }
        let reviewed = self.review_winget_package(
            &ReviewWingetPackage {
                version: CONTRACT_VERSION,
                package_id: request.package_id.clone(),
                package_version: request.package_version.clone(),
            },
            correlation_id,
        )?;
        if reviewed.review_hash != request.review_hash {
            return Err(api_error(
                "REVIEW_CHANGED",
                "Package review changed".into(),
                correlation_id,
            ));
        }
        let (details, provider_hash) = self
            .storage
            .show(&request.package_id, &request.package_version)
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        if digest(&format!(
            "{}\0{}\0{}\0{}",
            request.package_id, request.package_version, provider_hash, details
        )) != request.review_hash
        {
            return Err(api_error(
                "REVIEW_CHANGED",
                "Provider or package details changed".into(),
                correlation_id,
            ));
        }
        let before_packages = self
            .storage
            .installed_packages()
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        if before_packages.iter().any(|p| p.id == request.package_id) {
            return Err(api_error(
                "ALREADY_INSTALLED",
                "Package is already installed; associate it as an existing application".into(),
                correlation_id,
            ));
        }
        let before = self
            .storage
            .scan_installed()
            .map_err(|e| api_error("INVENTORY", e, correlation_id))?;
        let op_id = Uuid::new_v4().to_string();
        self.repository
            .begin_winget(
                &op_id,
                &request.cask_id,
                "INSTALL",
                &request.package_id,
                &request.package_version,
                now(correlation_id)?,
            )
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        started(&op_id);
        let outcome = self.storage.install(
            &request.package_id,
            &request.package_version,
            true,
            &|| self.repository.winget_cancelled(&op_id).unwrap_or(true),
            &|| {
                let _ = self.repository.winget_step(
                    &op_id,
                    "INSTALL",
                    now(correlation_id).unwrap_or(0),
                );
            },
        );
        if let Err(error) = outcome {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "PROVIDER_UNCERTAIN",
                now(correlation_id)?,
            );
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: {error}"),
                correlation_id,
            ));
        }
        if self.repository.winget_cancelled(&op_id).unwrap_or(true) {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "CANCELLED_AFTER_PROVIDER",
                now(correlation_id)?,
            );
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: cancellation requires inspection"),
                correlation_id,
            ));
        }
        let after_packages = self.storage.installed_packages().map_err(|e| {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "POST_PROVIDER_UNVERIFIED",
                now(correlation_id).unwrap_or(0),
            );
            api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: {e}"),
                correlation_id,
            )
        })?;
        let after = self.storage.scan_installed().map_err(|e| {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "POST_INVENTORY_UNVERIFIED",
                now(correlation_id).unwrap_or(0),
            );
            api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: {e}"),
                correlation_id,
            )
        })?;
        let name = reviewed_name(&reviewed.details, &request.package_id).unwrap_or_default();
        let candidates: Vec<_> = after
            .iter()
            .filter(|item| {
                !before
                    .iter()
                    .any(|old| old.installation_id == item.installation_id)
                    && item.name.eq_ignore_ascii_case(&name)
            })
            .collect();
        if matches_package(
            &after_packages,
            &request.package_id,
            &request.package_version,
        ) != 1
            || candidates.len() != 1
        {
            let _ =
                self.repository
                    .winget_uncertain(&op_id, "AMBIGUOUS_INSTALL", now(correlation_id)?);
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: installation could not be correlated exactly"),
                correlation_id,
            ));
        }
        let (_, after_provider_hash) = self
            .storage
            .show(&request.package_id, &request.package_version)
            .map_err(|e| {
                let _ = self.repository.winget_uncertain(
                    &op_id,
                    "POST_PROVIDER_CHANGED",
                    now(correlation_id).unwrap_or(0),
                );
                api_error(
                    "RECOVERY_REQUIRED",
                    format!("Operation {op_id}: {e}"),
                    correlation_id,
                )
            })?;
        if after_provider_hash != provider_hash {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "POST_PROVIDER_CHANGED",
                now(correlation_id)?,
            );
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: provider changed"),
                correlation_id,
            ));
        }
        self.repository
            .finish_winget_install(
                &op_id,
                &request.cask_id,
                &request.package_id,
                &request.package_version,
                &provider_hash,
                candidates[0],
                now(correlation_id)?,
            )
            .map_err(|e| {
                let _ = self.repository.winget_uncertain(
                    &op_id,
                    "REGISTRATION_FAILED",
                    now(correlation_id).unwrap_or(0),
                );
                api_error(
                    "RECOVERY_REQUIRED",
                    format!("Operation {op_id}: {e}"),
                    correlation_id,
                )
            })
    }
    /// # Errors
    /// Returns a persistence error.
    pub fn list_winget_installations(
        &self,
        request: &ListWingetInstallations,
        correlation_id: &str,
    ) -> Result<Vec<WingetInstallationView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list_winget_installations(&request.cask_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Returns a missing operation or persistence error.
    pub fn get_winget_operation(
        &self,
        request: &GetWingetOperation,
        correlation_id: &str,
    ) -> Result<WingetOperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .get_winget_operation(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Operation not found".into(), correlation_id))
    }
    /// # Errors
    /// Returns a persistence error.
    pub fn cancel_winget_operation(
        &self,
        request: &CancelWingetOperation,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .cancel_winget(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Closes uncertain work without asserting installation or uninstall success.
    pub fn close_winget_operation(
        &self,
        request: &CloseWingetOperation,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .close_winget_preserved(&request.id, now(correlation_id)?)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Returns a generated plan only when current provider and registry evidence agree.
    pub fn create_winget_uninstall_plan(
        &self,
        request: &CreateWingetUninstallPlan,
        correlation_id: &str,
    ) -> Result<WingetUninstallPlanView, ApiError> {
        check_version(request.version, correlation_id)?;
        let record = self
            .repository
            .get_winget_installation(&request.installation_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| {
                api_error(
                    "NOT_FOUND",
                    "Owned installation not found".into(),
                    correlation_id,
                )
            })?;
        if record.view.state != "INSTALLED" || record.view.provenance != "INSTALLED_BY_CASKLET" {
            return Err(api_error(
                "INELIGIBLE",
                "Installation is not verified and owned".into(),
                correlation_id,
            ));
        }
        if self
            .repository
            .winget_blocking_associations(
                &request.installation_id,
                &record.view.registry_installation_id,
            )
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            != 0
        {
            return Err(api_error(
                "SHARED",
                "Other associations retain this installation".into(),
                correlation_id,
            ));
        }
        let packages = self
            .storage
            .installed_packages()
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        let registry = self
            .storage
            .scan_installed()
            .map_err(|e| api_error("INVENTORY", e, correlation_id))?;
        let (_, provider_hash) = self
            .storage
            .show(&record.view.package_id, &record.view.package_version)
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        if matches_package(
            &packages,
            &record.view.package_id,
            &record.view.package_version,
        ) != 1
            || provider_hash != record.provider_sha256
            || !registry.iter().any(|item| {
                item.installation_id == record.view.registry_installation_id
                    && item.evidence_sha256 == record.registry_evidence_sha256
            })
        {
            return Err(api_error(
                "INELIGIBLE",
                "Provider or installed instance changed".into(),
                correlation_id,
            ));
        }
        let timestamp = now(correlation_id)?;
        let id = Uuid::new_v4().to_string();
        let expires = timestamp + 10 * 60 * 1000;
        let plan_hash = digest(&format!(
            "{}\0{}\0{}\0{}\0{}\0{}\0{}",
            id,
            record.view.id,
            record.view.package_id,
            record.view.package_version,
            record.registry_evidence_sha256,
            record.provider_sha256,
            record.revision
        ));
        let plan=WingetUninstallPlanView {version:CONTRACT_VERSION,id,installation_id:record.view.id.clone(),package_id:record.view.package_id.clone(),package_version:record.view.package_version.clone(),state:"DRAFT".into(),plan_hash,expires_at_ms:expires,warnings:vec!["Uninstall may affect other software using this global installation. Review all associations before approval.".into()]};
        self.repository
            .save_winget_plan(&plan, &record, timestamp)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        Ok(plan)
    }
    /// # Errors
    /// Approves an exact current plan.
    pub fn approve_winget_uninstall_plan(
        &self,
        request: &ApproveWingetUninstallPlan,
        correlation_id: &str,
    ) -> Result<WingetUninstallPlanView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .approve_winget_plan(&request.plan_id, &request.plan_hash, now(correlation_id)?)
            .map_err(|e| api_error("PLAN_INVALID", e, correlation_id))?;
        self.repository
            .get_winget_plan(&request.plan_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Plan missing".into(), correlation_id))
    }
    /// # Errors
    /// Runs only an approved revalidated plan and verifies absence afterward.
    #[cfg(feature = "reviewed-removal")]
    #[expect(
        clippy::too_many_lines,
        reason = "Uninstall rechecks every eligibility and external outcome boundary"
    )]
    pub fn execute_winget_uninstall_plan(
        &self,
        request: &ExecuteWingetUninstallPlan,
        correlation_id: &str,
        started: &dyn Fn(&str),
    ) -> Result<WingetUninstallPlanView, ApiError> {
        check_version(request.version, correlation_id)?;
        let plan = self
            .repository
            .get_winget_plan(&request.plan_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Plan missing".into(), correlation_id))?;
        if plan.state != "APPROVED"
            || plan.plan_hash != request.plan_hash
            || plan.expires_at_ms <= now(correlation_id)?
        {
            return Err(api_error(
                "PLAN_INVALID",
                "Plan is not approved or has expired".into(),
                correlation_id,
            ));
        }
        let record = self
            .repository
            .get_winget_installation(&plan.installation_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Installation missing".into(), correlation_id))?;
        if self
            .repository
            .winget_blocking_associations(&record.view.id, &record.view.registry_installation_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            != 0
        {
            return Err(api_error(
                "SHARED",
                "Other associations retain this installation".into(),
                correlation_id,
            ));
        }
        let packages = self
            .storage
            .installed_packages()
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        let registry = self
            .storage
            .scan_installed()
            .map_err(|e| api_error("INVENTORY", e, correlation_id))?;
        let (_, provider_hash) = self
            .storage
            .show(&plan.package_id, &plan.package_version)
            .map_err(|e| api_error("PROVIDER", e, correlation_id))?;
        if provider_hash != record.provider_sha256
            || matches_package(&packages, &plan.package_id, &plan.package_version) != 1
            || !registry.iter().any(|item| {
                item.installation_id == record.view.registry_installation_id
                    && item.evidence_sha256 == record.registry_evidence_sha256
            })
        {
            return Err(api_error(
                "PLAN_INVALID",
                "Installed instance changed".into(),
                correlation_id,
            ));
        }
        let op_id = Uuid::new_v4().to_string();
        self.repository
            .begin_winget_uninstall(&plan.id, &plan.plan_hash, &op_id, now(correlation_id)?)
            .map_err(|e| api_error("PLAN_INVALID", e, correlation_id))?;
        started(&op_id);
        if let Err(error) = self.storage.uninstall(
            &plan.package_id,
            &plan.package_version,
            &|| self.repository.winget_cancelled(&op_id).unwrap_or(true),
            &|| {
                let _ = self.repository.winget_step(
                    &op_id,
                    "UNINSTALL",
                    now(correlation_id).unwrap_or(0),
                );
            },
        ) {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "PROVIDER_UNCERTAIN",
                now(correlation_id)?,
            );
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: {error}"),
                correlation_id,
            ));
        }
        if self.repository.winget_cancelled(&op_id).unwrap_or(true) {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "CANCELLED_AFTER_PROVIDER",
                now(correlation_id)?,
            );
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: cancellation requires inspection"),
                correlation_id,
            ));
        }
        let after_packages = self.storage.installed_packages().map_err(|e| {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "POST_PROVIDER_UNVERIFIED",
                now(correlation_id).unwrap_or(0),
            );
            api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: {e}"),
                correlation_id,
            )
        })?;
        let after_registry = self.storage.scan_installed().map_err(|e| {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "POST_INVENTORY_UNVERIFIED",
                now(correlation_id).unwrap_or(0),
            );
            api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: {e}"),
                correlation_id,
            )
        })?;
        if after_packages.iter().any(|p| p.id == plan.package_id)
            || after_registry
                .iter()
                .any(|item| item.installation_id == record.view.registry_installation_id)
        {
            let _ = self.repository.winget_uncertain(
                &op_id,
                "UNINSTALL_UNVERIFIED",
                now(correlation_id)?,
            );
            return Err(api_error(
                "RECOVERY_REQUIRED",
                format!("Operation {op_id}: uninstall absence unverified"),
                correlation_id,
            ));
        }
        self.repository
            .finish_winget_uninstall(&plan.id, &op_id, now(correlation_id)?)
            .map_err(|e| {
                let _ = self.repository.winget_uncertain(
                    &op_id,
                    "REGISTRATION_FAILED",
                    now(correlation_id).unwrap_or(0),
                );
                api_error(
                    "RECOVERY_REQUIRED",
                    format!("Operation {op_id}: {e}"),
                    correlation_id,
                )
            })?;
        self.repository
            .get_winget_plan(&plan.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Plan missing".into(), correlation_id))
    }
}
