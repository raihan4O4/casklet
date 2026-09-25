//! Shared operation queries and conservative recovery commands.
use crate::{
    CaskRepository, CaskService, api_error, check_version,
    projection::{ManifestProjection, ProjectionRepository},
    resource::{IntakePaths, ResourceRepository, ResourceStorage},
};
use casklet_contracts::{
    ApiError, CONTRACT_VERSION, CancelOperation, GetOperation, ListOperations, OperationView,
    PreserveOperation, RecoverOperation, RecoveryInspection, RecoveryStatus,
};
use casklet_domain::resource::{IntakeMode, Resource, ResourceKind};

#[derive(Debug, Clone)]
pub struct RegisteredIntake {
    pub path: String,
    pub kind: ResourceKind,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct IntakeEvidence {
    pub mode: String,
    pub source_path: String,
    pub staging_path: String,
    pub final_path: String,
}

#[derive(Debug, Clone)]
pub struct PendingIntake {
    pub resource: Resource,
    pub mode: IntakeMode,
    pub source_path: String,
    pub paths: IntakePaths,
}

pub trait OperationRepository {
    /// # Errors
    /// Returns a persistence error.
    fn list_operations(&self, cask_id: Option<&str>) -> Result<Vec<OperationView>, String>;
    /// # Errors
    /// Returns a persistence error.
    fn get_operation(&self, id: &str) -> Result<Option<OperationView>, String>;
    /// # Errors
    /// Returns a persistence error.
    fn pending_manifests(&self) -> Result<Vec<String>, String>;
    /// # Errors
    /// Returns the committed target only when registration is complete.
    fn registered_intake(&self, id: &str) -> Result<Option<RegisteredIntake>, String>;
    /// # Errors
    /// Completes a previously verified registered intake.
    fn finish_registered_intake(&self, id: &str) -> Result<(), String>;
    /// # Errors
    /// Closes an abandoned operation while preserving all candidate files.
    fn preserve_operation(&self, id: &str) -> Result<(), String>;
    /// # Errors
    /// Returns stored recovery paths without treating them as ownership proof.
    fn intake_evidence(&self, id: &str) -> Result<Option<IntakeEvidence>, String>;
    /// # Errors
    /// Records a cancellation request for a running operation.
    fn request_cancellation(&self, id: &str) -> Result<(), String>;
    /// # Errors
    /// Returns an interrupted unregistered intake's stored snapshot.
    fn pending_intake(&self, id: &str) -> Result<Option<PendingIntake>, String>;
}

pub trait RecoveryFilesystem {
    fn path_status(&self, path: &str) -> String;
}

impl<
    R: OperationRepository + CaskRepository + ResourceRepository + ProjectionRepository,
    S: ResourceStorage + RecoveryFilesystem + ManifestProjection,
> CaskService<R, S>
{
    /// # Errors
    /// Returns a version or persistence error.
    pub fn list_operations(
        &self,
        request: &ListOperations,
        correlation_id: &str,
    ) -> Result<Vec<OperationView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list_operations(request.cask_id.as_deref())
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }

    /// # Errors
    /// Returns a version, persistence, or not-found error.
    pub fn get_operation(
        &self,
        request: &GetOperation,
        correlation_id: &str,
    ) -> Result<OperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .get_operation(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Operation not found".into(), correlation_id))
    }

    /// # Errors
    /// Returns a persistence error.
    pub fn recovery_status(&self, correlation_id: &str) -> Result<RecoveryStatus, ApiError> {
        let operations = self
            .repository
            .list_operations(None)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .into_iter()
            .filter(|item| item.state == "RECOVERY_REQUIRED")
            .collect();
        let pending_manifest_casks = self
            .repository
            .pending_manifests()
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        Ok(RecoveryStatus {
            version: CONTRACT_VERSION,
            operations,
            pending_manifest_casks,
        })
    }

    /// # Errors
    /// Returns a typed error if the operation is ambiguous or still active.
    pub fn recover_operation(
        &self,
        request: &RecoverOperation,
        correlation_id: &str,
    ) -> Result<OperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        let operation = self.get_operation(
            &GetOperation {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )?;
        if operation.state == "RUNNING" {
            return Err(api_error(
                "OPERATION_ACTIVE",
                "Operation may still be running".into(),
                correlation_id,
            ));
        }
        if operation.state == "SUCCEEDED" {
            return Ok(operation);
        }
        if operation.state != "RECOVERY_REQUIRED" {
            return Err(api_error(
                "OPERATION_STATE",
                "Operation is not awaiting recovery".into(),
                correlation_id,
            ));
        }
        if let Some(pending) = self
            .repository
            .pending_intake(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
        {
            self.recover_pending_intake(&request.id, &pending, correlation_id)?;
            return self.get_operation(
                &GetOperation {
                    version: request.version,
                    id: request.id.clone(),
                },
                correlation_id,
            );
        }
        let target = self
            .repository
            .registered_intake(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error(
                "RECOVERY_REQUIRED",
                "Intake is not fully registered; staged and final data have been preserved for inspection"
                    .into(),
                correlation_id,
            ))?;
        let observed = self
            .storage
            .inspect(&target.path, &|_, _| {})
            .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?;
        if observed.kind != target.kind
            || observed.size_bytes != target.size_bytes
            || observed.unknown_entries != 0
        {
            return Err(api_error(
                "RECOVERY_REQUIRED",
                "Registered resource no longer matches its recorded kind and size".into(),
                correlation_id,
            ));
        }
        self.repository
            .finish_registered_intake(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        self.get_operation(
            &GetOperation {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )
    }

    fn recover_pending_intake(
        &self,
        id: &str,
        pending: &PendingIntake,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        let cask = self
            .repository
            .get(&pending.resource.cask_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        let expected = self
            .storage
            .allocate(&cask, &pending.resource.id, &pending.resource.name);
        if expected.staging != pending.paths.staging
            || expected.final_path != pending.paths.final_path
        {
            return Err(api_error(
                "RECOVERY_REQUIRED",
                "Recorded managed paths no longer match this Cask".into(),
                correlation_id,
            ));
        }
        if pending.mode == IntakeMode::Move
            && self.storage.path_status(&pending.source_path) != "MISSING"
        {
            return Err(api_error(
                "RECOVERY_REQUIRED",
                "Move source is present or cannot be verified absent; files preserved".into(),
                correlation_id,
            ));
        }
        let stage = self.storage.path_status(&pending.paths.staging);
        let final_status = self.storage.path_status(&pending.paths.final_path);
        let recovered_identity = if stage.starts_with("PRESENT_") && final_status == "MISSING" {
            let observed =
                self.verify_recovery_item(&pending.paths.staging, pending, correlation_id)?;
            self.storage
                .finish_staged(&cask, &pending.resource, &pending.paths, &observed)
                .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?
        } else if stage != "MISSING" || !final_status.starts_with("PRESENT_") {
            return Err(api_error(
                "RECOVERY_REQUIRED",
                "Staging and managed destination are ambiguous; files preserved".into(),
                correlation_id,
            ));
        } else {
            None
        };
        let final_observation =
            self.verify_recovery_item(&pending.paths.final_path, pending, correlation_id)?;
        let mut resource = pending.resource.clone();
        resource.identity_key = recovered_identity.filter(|identity| {
            final_observation.recordable_identity.as_deref() == Some(identity.as_str())
        });
        self.repository
            .materialized_intake(id)
            .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?;
        self.repository
            .commit_resource(&resource, Some(id))
            .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?;
        self.repository
            .complete_intake(id)
            .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(())
    }

    fn verify_recovery_item(
        &self,
        path: &str,
        pending: &PendingIntake,
        correlation_id: &str,
    ) -> Result<crate::resource::Observation, ApiError> {
        let observed = self
            .storage
            .inspect(path, &|_, _| {})
            .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?;
        if observed.kind != pending.resource.kind
            || observed.size_bytes != pending.resource.size_bytes
            || observed.unknown_entries != 0
        {
            return Err(api_error(
                "RECOVERY_REQUIRED",
                "Candidate differs from recorded kind or size; files preserved".into(),
                correlation_id,
            ));
        }
        Ok(observed)
    }

    /// Leaves filesystem data untouched and releases an abandoned operation's Cask lock.
    /// # Errors
    /// Returns a typed error when the owner may still be active.
    pub fn preserve_operation(
        &self,
        request: &PreserveOperation,
        correlation_id: &str,
    ) -> Result<OperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .preserve_operation(&request.id)
            .map_err(|e| api_error("RECOVERY_REQUIRED", e, correlation_id))?;
        self.get_operation(
            &GetOperation {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )
    }

    /// Inspects stored paths without changing them.
    /// # Errors
    /// Returns a persistence or not-found error.
    pub fn inspect_recovery(
        &self,
        request: &GetOperation,
        correlation_id: &str,
    ) -> Result<RecoveryInspection, ApiError> {
        let operation = self.get_operation(request, correlation_id)?;
        let evidence = self
            .repository
            .intake_evidence(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| {
                api_error(
                    "NOT_FOUND",
                    "Intake evidence not found".into(),
                    correlation_id,
                )
            })?;
        Ok(RecoveryInspection {
            version: CONTRACT_VERSION,
            operation,
            mode: evidence.mode,
            source_status: self.storage.path_status(&evidence.source_path),
            staging_status: self.storage.path_status(&evidence.staging_path),
            final_status: self.storage.path_status(&evidence.final_path),
            source_path: evidence.source_path,
            staging_path: evidence.staging_path,
            final_path: evidence.final_path,
        })
    }

    /// Requests cancellation; the worker observes this at safe filesystem boundaries.
    /// # Errors
    /// Returns a typed state or persistence error.
    pub fn cancel_operation(
        &self,
        request: &CancelOperation,
        correlation_id: &str,
    ) -> Result<OperationView, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .request_cancellation(&request.id)
            .map_err(|e| api_error("OPERATION_STATE", e, correlation_id))?;
        self.get_operation(
            &GetOperation {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )
    }
}
