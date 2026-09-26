//! Portable ZIP use cases. Filesystem and process work stay behind a provider port.
use crate::{
    CaskRepository, CaskService, api_error, check_version,
    projection::{ManifestProjection, ProjectionRepository},
    resource::{ResourceRepository, ResourceStorage},
};
use casklet_contracts::{
    ApiError, CONTRACT_VERSION, GetPortableInstallation, InspectPortableZip, InstallPortableZip,
    LaunchPortableInstallation, ListPortableInstallations, PortableInstallationView,
    PortableZipPreview,
};
use casklet_domain::{
    Cask, CaskState,
    portable::PortableIdentity,
    resource::{IntakeMode, Resource, ResourceKind},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PortableInspection {
    pub sha256: String,
    pub file_count: u32,
    pub expanded_bytes: u64,
    pub executable_paths: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct PortablePaths {
    pub staging: String,
    pub final_path: String,
}
#[derive(Debug, Clone)]
pub struct PortableRecord {
    pub view: PortableInstallationView,
    pub executable_sha256: String,
    pub root_identity: String,
}

pub trait PortableProvider {
    /// # Errors
    /// Rejects unsafe or unsupported ZIP input.
    fn inspect_zip(&self, path: &str) -> Result<PortableInspection, String>;
    fn allocate_portable(
        &self,
        cask: &Cask,
        operation_id: &str,
        resource_id: &str,
    ) -> PortablePaths;
    /// # Errors
    /// Extracts only the reviewed archive into operation-owned staging.
    fn extract_zip(
        &self,
        path: &str,
        expected_hash: &str,
        selected_exe: &str,
        staging: &str,
        progress: &dyn Fn(u64, u64),
        cancel: &dyn Fn() -> bool,
    ) -> Result<(u64, String), String>;
    /// # Errors
    /// Finalizes a staged directory without overwriting another object.
    fn finalize_portable(&self, staging: &str, final_path: &str) -> Result<(), String>;
    /// # Errors
    /// Refuses a changed executable; starts the exact installed binary without a shell.
    fn launch_portable(
        &self,
        root: &str,
        relative_exe: &str,
        expected_hash: &str,
    ) -> Result<(), String>;
}

pub trait PortableRepository {
    /// # Errors
    /// Commits durable intent and a Cask mutation lock before extraction.
    #[expect(
        clippy::too_many_arguments,
        reason = "Durable intent records all reviewed install inputs"
    )]
    fn begin_portable(
        &self,
        op_id: &str,
        cask: &Cask,
        ids: &PortableIdentity,
        resource: &Resource,
        request: &InstallPortableZip,
        paths: &PortablePaths,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Atomically registers resource, application, installation, and association.
    #[expect(
        clippy::too_many_arguments,
        reason = "Atomic registration records all installation identities"
    )]
    fn commit_portable(
        &self,
        op_id: &str,
        cask: &Cask,
        ids: &PortableIdentity,
        resource: &Resource,
        request: &InstallPortableZip,
        executable_hash: &str,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Completes a registered installation and releases its lock.
    fn finish_portable(&self, op_id: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Marks uncertain work for recovery without deleting staging.
    fn fail_portable(&self, op_id: &str, code: &str) -> Result<(), String>;
    /// # Errors
    /// Returns instances associated with a Cask.
    fn list_portable(&self, cask_id: &str) -> Result<Vec<PortableInstallationView>, String>;
    /// # Errors
    /// Returns one instance with launch preconditions.
    fn get_portable(&self, id: &str) -> Result<Option<PortableRecord>, String>;
    /// # Errors
    /// Persists a typed launch outcome.
    fn record_launch(&self, id: &str, success: bool, now: i64) -> Result<(), String>;
}

fn now_ms(correlation_id: &str) -> Result<i64, ApiError> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
            .as_millis(),
    )
    .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))
}

impl<
    R: CaskRepository + ResourceRepository + ProjectionRepository + PortableRepository,
    S: ResourceStorage + ManifestProjection + PortableProvider,
> CaskService<R, S>
{
    /// # Errors
    /// Rejects an invalid or unsafe archive.
    pub fn inspect_portable_zip(
        &self,
        request: &InspectPortableZip,
        correlation_id: &str,
    ) -> Result<PortableZipPreview, ApiError> {
        check_version(request.version, correlation_id)?;
        let item = self
            .storage
            .inspect_zip(&request.archive_path)
            .map_err(|e| api_error("ARCHIVE_INVALID", e, correlation_id))?;
        Ok(PortableZipPreview {version:CONTRACT_VERSION,archive_sha256:item.sha256,file_count:item.file_count,
            expanded_bytes:item.expanded_bytes,executable_paths:item.executable_paths,
            warnings:vec!["Portable applications are not sandboxed. Inspect the source before choosing an executable to launch.".into()]})
    }

    /// # Errors
    /// Returns validation, extraction, persistence, or recoverable operation errors.
    #[expect(
        clippy::too_many_lines,
        reason = "durable install phases and verification form one use case"
    )]
    pub fn install_portable_zip(
        &self,
        request: &InstallPortableZip,
        correlation_id: &str,
        progress: &dyn Fn(u64, u64),
        started: &dyn Fn(&str),
    ) -> Result<PortableInstallationView, ApiError> {
        check_version(request.version, correlation_id)?;
        let name = request.application_name.trim();
        if name.is_empty()
            || name.chars().count() > 120
            || name.chars().any(char::is_control)
            || request
                .application_version
                .as_ref()
                .is_some_and(|v| v.len() > 80 || v.chars().any(char::is_control))
        {
            return Err(api_error(
                "INVALID_APPLICATION",
                "Invalid name or version".into(),
                correlation_id,
            ));
        }
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(api_error(
                "CASK_READ_ONLY",
                "Cask is not active".into(),
                correlation_id,
            ));
        }
        let inspected = self
            .storage
            .inspect_zip(&request.archive_path)
            .map_err(|e| api_error("ARCHIVE_INVALID", e, correlation_id))?;
        if inspected.sha256 != request.archive_sha256
            || !inspected
                .executable_paths
                .iter()
                .any(|path| path == &request.executable_relative_path)
        {
            return Err(api_error(
                "ARCHIVE_CHANGED",
                "ZIP or selected executable changed since preview".into(),
                correlation_id,
            ));
        }
        let ids = PortableIdentity::new();
        let op_id = Uuid::new_v4().to_string();
        let mut resource = Resource::new(
            cask.id.clone(),
            name.into(),
            String::new(),
            ResourceKind::Directory,
            IntakeMode::Copy,
            "applications".into(),
            Some(inspected.expanded_bytes),
            0,
            now_ms(correlation_id)?,
        )
        .map_err(|e| api_error("INVALID_APPLICATION", format!("{e:?}"), correlation_id))?;
        let paths = self.storage.allocate_portable(&cask, &op_id, &resource.id);
        resource.path.clone_from(&paths.final_path);
        self.repository
            .begin_portable(
                &op_id,
                &cask,
                &ids,
                &resource,
                request,
                &paths,
                resource.created_at_ms,
            )
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        if catch_unwind(AssertUnwindSafe(|| started(&op_id))).is_err() {
            let _ = self
                .repository
                .fail_portable(&op_id, "PRESENTATION_INTERRUPTED");
            return Err(api_error(
                "INSTALL_RECOVERY_REQUIRED",
                "Operation notification failed".into(),
                correlation_id,
            ));
        }
        let extracted = catch_unwind(AssertUnwindSafe(|| {
            self.storage.extract_zip(
                &request.archive_path,
                &request.archive_sha256,
                &request.executable_relative_path,
                &paths.staging,
                &|done, total| {
                    let _ = self.repository.progress_intake(&op_id, done, total);
                    progress(done, total);
                },
                &|| {
                    self.repository
                        .cancellation_requested(&op_id)
                        .unwrap_or(false)
                },
            )
        }));
        let (size, executable_hash) = match extracted {
            Ok(Ok(result)) => result,
            Ok(Err(error)) => {
                let _ = self.repository.fail_portable(&op_id, "EXTRACTION_FAILED");
                return Err(api_error(
                    "INSTALL_RECOVERY_REQUIRED",
                    error,
                    correlation_id,
                ));
            }
            Err(_) => {
                let _ = self.repository.fail_portable(&op_id, "WORKER_INTERRUPTED");
                return Err(api_error(
                    "INSTALL_RECOVERY_REQUIRED",
                    "Worker interrupted".into(),
                    correlation_id,
                ));
            }
        };
        if size != inspected.expanded_bytes {
            let _ = self.repository.fail_portable(&op_id, "SIZE_CHANGED");
            return Err(api_error(
                "INSTALL_RECOVERY_REQUIRED",
                "Expanded size changed".into(),
                correlation_id,
            ));
        }
        if let Err(error) = self
            .storage
            .finalize_portable(&paths.staging, &paths.final_path)
        {
            let _ = self.repository.fail_portable(&op_id, "FINALIZE_FAILED");
            return Err(api_error(
                "INSTALL_RECOVERY_REQUIRED",
                error,
                correlation_id,
            ));
        }
        let observed = self
            .storage
            .inspect(&paths.final_path, &|_, _| {})
            .map_err(|e| {
                let _ = self
                    .repository
                    .fail_portable(&op_id, "FINAL_VERIFICATION_FAILED");
                api_error("INSTALL_RECOVERY_REQUIRED", e, correlation_id)
            })?;
        if observed.kind != ResourceKind::Directory
            || observed.size_bytes != Some(size)
            || observed.unknown_entries != 0
            || observed.recordable_identity.is_none()
        {
            let _ = self
                .repository
                .fail_portable(&op_id, "FINAL_VERIFICATION_FAILED");
            return Err(api_error(
                "INSTALL_RECOVERY_REQUIRED",
                "Installed tree could not be verified".into(),
                correlation_id,
            ));
        }
        resource.identity_key = observed.recordable_identity;
        self.repository
            .commit_portable(
                &op_id,
                &cask,
                &ids,
                &resource,
                request,
                &executable_hash,
                now_ms(correlation_id)?,
            )
            .map_err(|e| {
                let _ = self.repository.fail_portable(&op_id, "REGISTRATION_FAILED");
                api_error("INSTALL_RECOVERY_REQUIRED", e, correlation_id)
            })?;
        self.repository
            .finish_portable(&op_id, now_ms(correlation_id)?)
            .map_err(|e| api_error("INSTALL_RECOVERY_REQUIRED", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        self.repository
            .get_portable(&ids.installation_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .map(|r| r.view)
            .ok_or_else(|| {
                api_error(
                    "PERSISTENCE",
                    "Installation disappeared".into(),
                    correlation_id,
                )
            })
    }

    /// # Errors
    /// Returns a version or persistence error.
    pub fn list_portable_installations(
        &self,
        request: &ListPortableInstallations,
        correlation_id: &str,
    ) -> Result<Vec<PortableInstallationView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list_portable(&request.cask_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Returns a version, not-found, or persistence error.
    pub fn get_portable_installation(
        &self,
        request: &GetPortableInstallation,
        correlation_id: &str,
    ) -> Result<PortableInstallationView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .get_portable(&request.installation_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .map(|r| r.view)
            .ok_or_else(|| api_error("NOT_FOUND", "Installation not found".into(), correlation_id))
    }
    /// # Errors
    /// Refuses changed, missing, unverified, or unassociated installations.
    pub fn launch_portable_installation(
        &self,
        request: &LaunchPortableInstallation,
        correlation_id: &str,
    ) -> Result<PortableInstallationView, ApiError> {
        check_version(request.version, correlation_id)?;
        let record = self
            .repository
            .get_portable(&request.installation_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| {
                api_error("NOT_FOUND", "Installation not found".into(), correlation_id)
            })?;
        if !record.view.can_launch || record.view.state != "INSTALLED" {
            return Err(api_error(
                "LAUNCH_UNAVAILABLE",
                "Installation is not verified for launch".into(),
                correlation_id,
            ));
        }
        let observed = self
            .storage
            .inspect(&record.view.root_path, &|_, _| {})
            .map_err(|e| api_error("LAUNCH_UNAVAILABLE", e, correlation_id))?;
        if observed.recordable_identity.as_deref() != Some(&record.root_identity) {
            return Err(api_error(
                "INSTALLATION_CHANGED",
                "Installed tree changed since registration".into(),
                correlation_id,
            ));
        }
        let result = self.storage.launch_portable(
            &record.view.root_path,
            &record.view.executable_relative_path,
            &record.executable_sha256,
        );
        self.repository
            .record_launch(
                &request.installation_id,
                result.is_ok(),
                now_ms(correlation_id)?,
            )
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        result.map_err(|e| api_error("LAUNCH_FAILED", e, correlation_id))?;
        Ok(record.view)
    }
}
