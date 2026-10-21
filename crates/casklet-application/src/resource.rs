//! Resource intake use cases. Filesystem behavior remains behind a port.
use crate::{
    CaskRepository, CaskService,
    projection::{ManifestProjection, ProjectionRepository},
};
use casklet_contracts::{
    AddMode, AddResource, ApiError, CONTRACT_VERSION, CategorizeResource, CategoryView,
    CreateCategory, GetResource, GetStorageSummary, ListCategories, ListResources, ResourceView,
    StorageSummary,
};
use casklet_domain::{
    Cask, CaskState,
    resource::{IntakeMode, Resource, ResourceKind},
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Observation {
    pub source_path: String,
    pub name: String,
    pub kind: ResourceKind,
    pub size_bytes: Option<u64>,
    pub unknown_entries: u64,
    pub identity: String,
    pub recordable_identity: Option<String>,
    pub file_identity: Arc<dyn FileIdentity>,
}

pub trait FileIdentity: std::fmt::Debug + Send + Sync {
    /// # Errors
    /// Returns an I/O error when the path cannot be inspected.
    fn matches_path(&self, path: &str) -> Result<bool, String>;
}

#[derive(Debug, Clone)]
pub struct IntakePaths {
    pub staging: String,
    pub final_path: String,
}

#[derive(Debug, Clone)]
pub struct ApplicationStorageEntry {
    pub source_key: String,
    pub evidence_sha256: String,
    pub owned: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ApplicationSize {
    Missing,
    Known { bytes: u64, estimated: bool },
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ResourceMeasurement {
    pub availability: String,
    pub size_bytes: Option<u64>,
    pub unknown_entries: u64,
    pub total_entries: u64,
    pub entries: Vec<casklet_contracts::ResourceContentEntry>,
}

pub trait ResourceRepository {
    /// # Errors
    /// Reads persisted presentation sizes only; must never inspect filesystem contents.
    fn cached_application_sizes(
        &self,
        entries: &[ApplicationStorageEntry],
    ) -> Result<Vec<ApplicationSize>, String>;
    /// # Errors
    /// Saves observations bound to current registration evidence, without ownership changes.
    fn record_application_sizes(
        &self,
        entries: &[ApplicationStorageEntry],
        sizes: &[ApplicationSize],
    ) -> Result<(), String>;

    /// # Errors
    /// Reads original object evidence independently of current size observations.
    fn recorded_resource_identity(&self, _id: &str) -> Result<Option<String>, String> {
        Ok(None)
    }
    /// # Errors
    /// Saves read-only measurements without altering identity or granting authority.
    fn record_measurement(&self, _resource: &ResourceView) -> Result<(), String> {
        Ok(())
    }
    /// # Errors
    /// Reads active application associations and installer resource identities.
    fn application_storage_entries(
        &self,
        cask_id: &str,
    ) -> Result<Vec<ApplicationStorageEntry>, String>;
    /// # Errors
    /// Returns typed installer artifacts, which are not installed applications.
    fn installer_resource_ids(&self, cask_id: &str) -> Result<Vec<String>, String>;
    /// # Errors
    /// Atomically records explicit in-place authority, reusing the exact local item ID.
    /// Rejects locks and other overlapping dependencies; returns the persisted resource ID.
    fn commit_adoption(
        &self,
        resource: &Resource,
        expected: Option<&str>,
    ) -> Result<String, String>;
    /// # Errors
    /// Changes an exact association policy after optimistic state and lock checks.
    fn commit_ownership_policy(
        &self,
        resource: &Resource,
        expected: &str,
        owned: bool,
    ) -> Result<String, String>;
    /// # Errors
    /// Returns a persistence error.
    fn category_exists(&self, cask_id: &str, key: &str) -> Result<bool, String>;
    /// # Errors
    /// Returns a persistence error.
    fn create_category(&self, cask_id: &str, key: &str, name: &str) -> Result<(), String>;
    /// # Errors
    /// Returns a persistence error.
    fn list_categories(&self, cask_id: &str) -> Result<Vec<CategoryView>, String>;
    /// # Errors
    /// Returns a persistence error.
    fn begin_intake(
        &self,
        op_id: &str,
        resource: &Resource,
        mode: IntakeMode,
        source: &str,
        staging: &str,
        final_path: &str,
    ) -> Result<(), String>;
    /// # Errors
    /// Returns a persistence error.
    /// Reference retries reuse the active same-Cask path; returns the persisted resource ID.
    fn commit_resource(&self, resource: &Resource, op_id: Option<&str>) -> Result<String, String>;
    /// # Errors
    /// Returns a persistence error.
    fn complete_intake(&self, op_id: &str) -> Result<(), String>;
    /// # Errors
    /// Returns a persistence error. Progress is advisory; the durable state remains authoritative.
    fn progress_intake(&self, op_id: &str, completed: u64, total: u64) -> Result<(), String>;
    /// # Errors
    /// Records an uncertain intake without discarding its staging data.
    fn fail_intake(&self, op_id: &str, error_code: &str) -> Result<(), String>;
    /// # Errors
    /// Records the materialization step before registration begins.
    fn materialized_intake(&self, op_id: &str) -> Result<(), String>;
    /// # Errors
    /// Returns whether cancellation has been requested for this operation.
    fn cancellation_requested(&self, op_id: &str) -> Result<bool, String>;
    /// # Errors
    /// Returns a persistence error.
    fn list_resources(&self, cask_id: &str) -> Result<Vec<ResourceView>, String>;
    /// # Errors
    /// Returns a persistence error.
    fn get_resource(&self, id: &str) -> Result<Option<ResourceView>, String>;
    /// # Errors
    /// Returns a persistence error.
    fn categorize(&self, id: &str, key: &str) -> Result<(), String>;
}

pub trait ResourceStorage {
    /// Compares stable directory objects without hashing descendants. None means
    /// the legacy evidence or platform cannot make this comparison.
    fn same_directory_object(&self, _path: &str, _recorded: &str) -> Option<bool> {
        None
    }
    /// Metadata-only reconciliation; adapters may cache unchanged directory listings.
    fn measure_resource(
        &self,
        _path: &str,
        _kind: ResourceKind,
        _force: bool,
    ) -> Option<ResourceMeasurement> {
        None
    }
    /// # Errors
    /// Returns current read-only application size observations, independently of ownership.
    fn application_sizes(
        &self,
        entries: &[ApplicationStorageEntry],
    ) -> Result<Vec<ApplicationSize>, String>;
    /// # Errors
    /// Validates a previously registered internal allocation for renewed ownership.
    fn validate_internal_location(
        &self,
        _cask: &Cask,
        _observed: &Observation,
    ) -> Result<(), String> {
        Err("Internal ownership changes are unsupported".into())
    }
    /// # Errors
    /// Rejects broad, linked, protected, or insufficiently verified external locations.
    fn validate_owned_location(&self, _observed: &Observation) -> Result<(), String> {
        Err("In-place ownership is not supported by this storage adapter".into())
    }
    /// # Errors
    /// Returns a filesystem validation or inspection error.
    fn inspect(&self, source: &str, progress: &dyn Fn(u64, u64)) -> Result<Observation, String>;
    /// # Errors
    /// References require path/object metadata, never content digests or tree enumeration.
    fn inspect_reference(
        &self,
        source: &str,
        progress: &dyn Fn(u64, u64),
    ) -> Result<Observation, String>;
    fn availability(&self, path: &str, kind: ResourceKind) -> &'static str;
    fn allocate(&self, cask: &Cask, resource_id: &str, name: &str) -> IntakePaths;
    /// # Errors
    /// Returns a copy or verification error. Staging is retained for recovery.
    fn materialize(
        &self,
        observed: &Observation,
        paths: &IntakePaths,
        mode: IntakeMode,
        progress: &dyn Fn(u64, u64),
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<String>, String>;
    /// # Errors
    /// Renames a verified staged item into its recorded managed destination.
    fn finish_staged(
        &self,
        cask: &Cask,
        resource: &Resource,
        paths: &IntakePaths,
        observed: &Observation,
    ) -> Result<Option<String>, String>;
}

/// Resolves the shared intake default; explicit categories and folder handling stay intact.
fn intake_category<'a>(requested: &'a str, name: &str, kind: ResourceKind) -> &'a str {
    if requested != "auto" {
        return requested;
    }
    if kind == ResourceKind::Directory {
        return "other";
    }
    let name = name.to_lowercase();
    let extension = std::path::Path::new(&name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    match extension {
        "exe" | "msi" | "msix" | "appx" | "msixbundle" => "applications",
        "pdf" | "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "csv" | "xls" | "xlsx" | "ods"
        | "ppt" | "pptx" | "odp" | "epub" => "documents",
        "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "svg" | "heic"
        | "avif" | "ico" => "images",
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "wmv" | "m4v" | "mpeg" | "mpg" => "videos",
        "mp3" | "wav" | "flac" | "aac" | "ogg" | "opus" | "m4a" | "wma" | "aiff" => "audio",
        "zip" | "7z" | "rar" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "tbz2" | "zst" => "archives",
        _ => "other",
    }
}

impl<
    R: CaskRepository + ResourceRepository + ProjectionRepository,
    S: ResourceStorage + ManifestProjection,
> CaskService<R, S>
{
    /// Registers explicit cleanup authority without moving or copying user data.
    /// # Errors
    /// Rejects missing consent, inactive Casks, unsafe locations, and overlapping associations.
    #[expect(
        clippy::too_many_lines,
        reason = "intake validation, identity and authority registration form one auditable use case"
    )]
    pub fn adopt_resource(
        &self,
        request: &casklet_contracts::AdoptResource,
        correlation_id: &str,
        progress: &dyn Fn(u64, u64),
    ) -> Result<ResourceView, ApiError> {
        check(request.version, correlation_id)?;
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(err(
                "CASK_READ_ONLY",
                "Cask is not active".into(),
                correlation_id,
            ));
        }
        let observed = self
            .storage
            .inspect(&request.source_path, progress)
            .map_err(|e| err("SOURCE", e, correlation_id))?;
        let category_key =
            intake_category(&request.category_key, &observed.name, observed.kind).to_string();
        if !self
            .repository
            .category_exists(&cask.id, &category_key)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
        {
            return Err(err(
                "INVALID_CATEGORY",
                "Category is unavailable".into(),
                correlation_id,
            ));
        }
        if let Some(existing) = self
            .repository
            .list_resources(&cask.id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .into_iter()
            .find(|item| {
                observed
                    .file_identity
                    .matches_path(&item.path)
                    .unwrap_or(false)
            })
        {
            return self.add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    source_path: observed.source_path,
                    mode: AddMode::Reference,
                    category_key: existing.category_key,
                },
                correlation_id,
                progress,
            );
        }
        if !request.confirm_ownership {
            return Err(err("OWNERSHIP_CONSENT", "Confirm this data is dedicated to this activity and may be cleaned up after plan approval".into(), correlation_id));
        }

        self.storage
            .validate_owned_location(&observed)
            .map_err(|e| err("UNSAFE_OWNERSHIP", e, correlation_id))?;
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| err("CLOCK", e.to_string(), correlation_id))?
                .as_millis(),
        )
        .map_err(|e| err("CLOCK", e.to_string(), correlation_id))?;
        let mut resource = Resource::new(
            cask.id,
            observed.name,
            observed.source_path.clone(),
            observed.kind,
            IntakeMode::Adopt,
            category_key,
            observed.size_bytes,
            observed.unknown_entries,
            now,
        )
        .map_err(|e| err("INVALID_RESOURCE", format!("{e:?}"), correlation_id))?;
        resource.identity_key = observed.recordable_identity;
        if !observed
            .file_identity
            .matches_path(&resource.path)
            .unwrap_or(false)
        {
            return Err(err(
                "SOURCE_CHANGED",
                "Source changed during ownership review".into(),
                correlation_id,
            ));
        }
        resource.id = self
            .repository
            .commit_adoption(&resource, None)
            .map_err(|e| err("OWNERSHIP_CONFLICT", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        self.get_resource(
            &GetResource {
                version: CONTRACT_VERSION,
                id: resource.id,
            },
            correlation_id,
        )
    }
    /// Explicitly changes policy without moving or modifying file bytes.
    /// # Errors
    /// Rejects stale policy, missing consent, unsafe ownership and busy Casks.
    #[expect(
        clippy::too_many_lines,
        reason = "explicit consent, source validation and policy commit form one auditable use case"
    )]
    pub fn set_resource_ownership(
        &self,
        request: &casklet_contracts::SetResourceOwnership,
        correlation_id: &str,
    ) -> Result<ResourceView, ApiError> {
        check(request.version, correlation_id)?;
        let current = self
            .repository
            .list_resources(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .into_iter()
            .find(|item| item.id == request.resource_id)
            .ok_or_else(|| {
                err(
                    "NOT_FOUND",
                    "Item not found in this Cask".into(),
                    correlation_id,
                )
            })?;
        if current.ownership != request.expected_ownership {
            return Err(err(
                "OWNERSHIP_CHANGED",
                "Ownership changed; refresh this item and try again".into(),
                correlation_id,
            ));
        }
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .filter(|cask| cask.state == CaskState::Active)
            .ok_or_else(|| {
                err(
                    "CASK_READ_ONLY",
                    "Cask is unavailable or not active".into(),
                    correlation_id,
                )
            })?;
        let kind = ResourceKind::from_code(&current.kind).ok_or_else(|| {
            err(
                "INVALID_RESOURCE",
                "Unsupported item kind".into(),
                correlation_id,
            )
        })?;
        let mut resource = Resource::new(
            cask.id.clone(),
            current.name,
            current.path.clone(),
            kind,
            IntakeMode::Adopt,
            current.category_key,
            current.size_bytes,
            current.unknown_entries,
            i64::try_from(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| err("CLOCK", e.to_string(), correlation_id))?
                    .as_millis(),
            )
            .map_err(|e| err("CLOCK", e.to_string(), correlation_id))?,
        )
        .map_err(|e| err("INVALID_RESOURCE", format!("{e:?}"), correlation_id))?;
        resource.id = current.id;
        if request.owned {
            if !request.confirm_ownership {
                return Err(err("OWNERSHIP_CONSENT", "Confirm this item is dedicated to this Cask and may be removed by reviewed cleanup".into(), correlation_id));
            }
            let observed = self
                .storage
                .inspect(&current.path, &|_, _| {})
                .map_err(|e| err("SOURCE", e, correlation_id))?;
            if observed.kind != kind {
                return Err(err(
                    "SOURCE_CHANGED",
                    "Item kind changed; inspect it before changing ownership".into(),
                    correlation_id,
                ));
            }
            if current.containment == "INTERNAL" {
                self.storage.validate_internal_location(&cask, &observed)
            } else {
                self.storage.validate_owned_location(&observed)
            }
            .map_err(|e| err("UNSAFE_OWNERSHIP", e, correlation_id))?;
            if !observed
                .file_identity
                .matches_path(&current.path)
                .unwrap_or(false)
            {
                return Err(err(
                    "SOURCE_CHANGED",
                    "Item changed during ownership review".into(),
                    correlation_id,
                ));
            }
            resource.identity_key = observed.recordable_identity;
            resource.size_bytes = observed.size_bytes;
            resource.unknown_entries = observed.unknown_entries;
        }
        let id = self
            .repository
            .commit_ownership_policy(&resource, &request.expected_ownership, request.owned)
            .map_err(|e| err("OWNERSHIP_CONFLICT", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        let mut updated = self
            .repository
            .list_resources(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| {
                err(
                    "NOT_FOUND",
                    "Item is no longer associated".into(),
                    correlation_id,
                )
            })?;
        self.refresh_availability(&mut updated, correlation_id)?;
        Ok(updated)
    }
    /// # Errors
    /// Returns validation, persistence, or filesystem errors with a correlation ID.
    pub fn add_resource(
        &self,
        request: &AddResource,
        correlation_id: &str,
        progress: &dyn Fn(u64, u64),
    ) -> Result<ResourceView, ApiError> {
        self.add_resource_with_events(request, correlation_id, progress, &|_| {})
    }

    /// User-facing intake policy; legacy/provider copy and move implementation stays available below.
    /// # Errors
    /// Copy and Move are temporarily disabled by ADR 0020.
    pub fn add_user_resource_with_events(
        &self,
        request: &AddResource,
        correlation_id: &str,
        progress: &dyn Fn(u64, u64),
        started: &dyn Fn(&str),
    ) -> Result<ResourceView, ApiError> {
        if !matches!(request.mode, AddMode::Reference) {
            return Err(err("INTAKE_MODE_DISABLED", "Copy and Move are temporarily disabled. Use Reference or Manage at current location.".into(), correlation_id));
        }
        self.add_resource_with_events(request, correlation_id, progress, started)
    }

    /// Reports the durable operation ID after intent and lock are committed.
    /// # Errors
    /// Returns validation, persistence, or filesystem errors with a correlation ID.
    #[expect(
        clippy::too_many_lines,
        reason = "intake validation and durable phases form one use case"
    )]
    pub fn add_resource_with_events(
        &self,
        request: &AddResource,
        correlation_id: &str,
        progress: &dyn Fn(u64, u64),
        started: &dyn Fn(&str),
    ) -> Result<ResourceView, ApiError> {
        check(request.version, correlation_id)?;
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(err(
                "CASK_READ_ONLY",
                "Cask is not active".into(),
                correlation_id,
            ));
        }
        let mode = match request.mode {
            AddMode::Copy => IntakeMode::Copy,
            AddMode::Move => IntakeMode::Move,
            AddMode::Reference => IntakeMode::Reference,
        };
        let observed = if mode == IntakeMode::Reference {
            self.storage
                .inspect_reference(&request.source_path, progress)
        } else {
            self.storage.inspect(&request.source_path, progress)
        }
        .map_err(|e| err("SOURCE", e, correlation_id))?;
        let category_key =
            intake_category(&request.category_key, &observed.name, observed.kind).to_string();
        if !self
            .repository
            .category_exists(&cask.id, &category_key)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
        {
            return Err(err(
                "INVALID_CATEGORY",
                "Category is unavailable for this Cask".into(),
                correlation_id,
            ));
        }

        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| err("CLOCK", e.to_string(), correlation_id))?
                .as_millis(),
        )
        .map_err(|e| err("CLOCK", e.to_string(), correlation_id))?;
        let mut resource = Resource::new(
            cask.id.clone(),
            observed.name.clone(),
            observed.source_path.clone(),
            observed.kind,
            mode,
            category_key,
            observed.size_bytes,
            observed.unknown_entries,
            now,
        )
        .map_err(|e| err("INVALID_RESOURCE", format!("{e:?}"), correlation_id))?;
        if mode == IntakeMode::Reference {
            resource
                .identity_key
                .clone_from(&observed.recordable_identity);
            if let Some(existing) = self
                .repository
                .list_resources(&cask.id)
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?
                .into_iter()
                .find(|item| {
                    observed
                        .file_identity
                        .matches_path(&item.path)
                        .unwrap_or(false)
                })
            {
                resource.id = existing.id;
            }
            resource.id = self
                .repository
                .commit_resource(&resource, None)
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        } else {
            let paths = self.storage.allocate(&cask, &resource.id, &resource.name);
            resource.path.clone_from(&paths.final_path);
            let op_id = Uuid::new_v4().to_string();
            self.repository
                .begin_intake(
                    &op_id,
                    &resource,
                    mode,
                    &observed.source_path,
                    &paths.staging,
                    &paths.final_path,
                )
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
            if catch_unwind(AssertUnwindSafe(|| started(&op_id))).is_err() {
                let _ = self
                    .repository
                    .fail_intake(&op_id, "PRESENTATION_INTERRUPTED");
                return Err(err(
                    "INTAKE_RECOVERY_REQUIRED",
                    "Operation notification failed; intent preserved".into(),
                    correlation_id,
                ));
            }
            let materialized = catch_unwind(AssertUnwindSafe(|| {
                self.storage.materialize(
                    &observed,
                    &paths,
                    mode,
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
            let Ok(materialized) = materialized else {
                let _ = self.repository.fail_intake(&op_id, "WORKER_INTERRUPTED");
                return Err(err(
                    "INTAKE_RECOVERY_REQUIRED",
                    "Worker interrupted; staged data preserved".into(),
                    correlation_id,
                ));
            };
            let materialized_identity = match materialized {
                Ok(identity) => identity,
                Err(error) => {
                    let code = if self
                        .repository
                        .cancellation_requested(&op_id)
                        .unwrap_or(false)
                    {
                        "CANCELLED_STAGING_PRESERVED"
                    } else {
                        "MATERIALIZE_FAILED"
                    };
                    let _ = self.repository.fail_intake(&op_id, code);
                    return Err(err("INTAKE_RECOVERY_REQUIRED", error, correlation_id));
                }
            };
            if let Err(error) = self.repository.materialized_intake(&op_id) {
                let _ = self.repository.fail_intake(&op_id, "STEP_JOURNAL_FAILED");
                return Err(err("INTAKE_RECOVERY_REQUIRED", error, correlation_id));
            }
            let final_observation = match self.storage.inspect(&paths.final_path, &|_, _| {}) {
                Ok(value) => value,
                Err(error) => {
                    let _ = self
                        .repository
                        .fail_intake(&op_id, "FINAL_VERIFICATION_FAILED");
                    return Err(err("INTAKE_RECOVERY_REQUIRED", error, correlation_id));
                }
            };
            if final_observation.kind != resource.kind
                || final_observation.size_bytes != resource.size_bytes
                || final_observation.unknown_entries != 0
                || final_observation.recordable_identity != materialized_identity
            {
                let _ = self
                    .repository
                    .fail_intake(&op_id, "FINAL_VERIFICATION_FAILED");
                return Err(err(
                    "INTAKE_RECOVERY_REQUIRED",
                    "Final item changed; recovery required".into(),
                    correlation_id,
                ));
            }
            resource.identity_key = final_observation.recordable_identity;
            if let Err(error) = self.repository.commit_resource(&resource, Some(&op_id)) {
                let _ = self.repository.fail_intake(&op_id, "REGISTRATION_FAILED");
                return Err(err("INTAKE_RECOVERY_REQUIRED", error, correlation_id));
            }
            if let Err(error) = self.repository.complete_intake(&op_id) {
                let _ = self.repository.fail_intake(&op_id, "COMPLETION_FAILED");
                return Err(err("INTAKE_RECOVERY_REQUIRED", error, correlation_id));
            }
        }
        self.repository
            .get_resource(&resource.id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| {
                err(
                    "PERSISTENCE",
                    "Resource disappeared after registration".into(),
                    correlation_id,
                )
            })
            .inspect(|_| {
                let _ = self.retry_manifest_projections(correlation_id);
            })
    }

    /// # Errors
    /// Returns a typed query or persistence error.
    pub fn list_resource_metadata(
        &self,
        request: &ListResources,
        correlation_id: &str,
    ) -> Result<Vec<ResourceView>, ApiError> {
        check(request.version, correlation_id)?;
        self.repository
            .list_resources(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))
    }
    /// Explicit live inspection, outside the ordinary presentation loading path.
    /// # Errors
    /// Returns a validation, inspection or persistence error.
    pub fn list_resources(
        &self,
        request: &ListResources,
        correlation_id: &str,
    ) -> Result<Vec<ResourceView>, ApiError> {
        check(request.version, correlation_id)?;
        let mut resources = self
            .repository
            .list_resources(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        for resource in &mut resources {
            self.refresh_availability(resource, correlation_id)?;
        }
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(resources)
    }
    /// # Errors
    /// Returns a typed query or persistence error.
    pub fn get_resource(
        &self,
        request: &GetResource,
        correlation_id: &str,
    ) -> Result<ResourceView, ApiError> {
        check(request.version, correlation_id)?;
        let mut resource = self
            .repository
            .get_resource(&request.id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Resource not found".into(), correlation_id))?;
        self.refresh_availability(&mut resource, correlation_id)?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(resource)
    }

    /// Lists a bounded current projection of folder contents. Entries inherit the
    /// parent's association; they do not receive independent deletion grants.
    /// # Errors
    /// Returns a validation, storage capability or persistence error.
    pub fn resource_contents(
        &self,
        request: &GetResource,
        refresh: bool,
        correlation_id: &str,
    ) -> Result<casklet_contracts::ResourceContents, ApiError> {
        let mut resource = self.get_resource(request, correlation_id)?;
        let kind = ResourceKind::from_code(&resource.kind)
            .ok_or_else(|| err("PERSISTENCE", "Invalid kind".into(), correlation_id))?;
        let measured = self
            .storage
            .measure_resource(&resource.path, kind, refresh)
            .ok_or_else(|| {
                err(
                    "UNSUPPORTED",
                    "Content inspection unavailable".into(),
                    correlation_id,
                )
            })?;
        resource.availability.clone_from(&measured.availability);
        resource.size_bytes = measured.size_bytes;
        resource.unknown_entries = measured.unknown_entries;
        resource.size_measurement = if measured.size_bytes.is_none() {
            "UNKNOWN"
        } else if kind == ResourceKind::Directory {
            "CALCULATED"
        } else {
            "EXACT"
        }
        .into();
        self.refresh_directory_identity(&mut resource, correlation_id)?;
        self.repository
            .record_measurement(&resource)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(casklet_contracts::ResourceContents {
            resource_id: resource.id,
            ownership: if resource.ownership == "OWNED" && resource.availability == "CHANGED" {
                "UNVERIFIED".into()
            } else {
                resource.ownership
            },
            availability: resource.availability,
            total_entries: measured.total_entries,
            unknown_entries: measured.unknown_entries,
            entries: measured.entries,
        })
    }
    /// # Errors
    /// Returns a typed validation or persistence error.
    pub fn create_category(
        &self,
        request: &CreateCategory,
        correlation_id: &str,
    ) -> Result<CategoryView, ApiError> {
        check(request.version, correlation_id)?;
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(err(
                "CASK_READ_ONLY",
                "Cask is not active".into(),
                correlation_id,
            ));
        }
        let name = request.name.trim();
        if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
            return Err(err(
                "INVALID_CATEGORY",
                "Category name must contain 1–80 non-control characters".into(),
                correlation_id,
            ));
        }
        let key = format!("custom:{}", Uuid::new_v4());
        self.repository
            .create_category(&cask.id, &key, name)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(CategoryView {
            version: CONTRACT_VERSION,
            key,
            name: name.into(),
            is_builtin: false,
        })
    }
    /// # Errors
    /// Returns a typed query or persistence error.
    pub fn list_categories(
        &self,
        request: &ListCategories,
        correlation_id: &str,
    ) -> Result<Vec<CategoryView>, ApiError> {
        check(request.version, correlation_id)?;
        self.repository
            .list_categories(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Returns a typed validation or persistence error.
    pub fn categorize_resource(
        &self,
        request: &CategorizeResource,
        correlation_id: &str,
    ) -> Result<ResourceView, ApiError> {
        check(request.version, correlation_id)?;
        let resource = self
            .repository
            .get_resource(&request.resource_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Resource not found".into(), correlation_id))?;
        let cask = self
            .repository
            .get(&resource.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.state != CaskState::Active {
            return Err(err(
                "CASK_READ_ONLY",
                "Cask is not active".into(),
                correlation_id,
            ));
        }
        if !self
            .repository
            .category_exists(&resource.cask_id, &request.category_key)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
        {
            return Err(err(
                "INVALID_CATEGORY",
                "Category is unavailable for this Cask".into(),
                correlation_id,
            ));
        }
        self.repository
            .categorize(&request.resource_id, &request.category_key)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        self.repository
            .get_resource(&request.resource_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| err("NOT_FOUND", "Resource not found".into(), correlation_id))
    }
    /// # Errors
    /// Returns a typed query or persistence error.
    pub fn storage_summary(
        &self,
        request: &GetStorageSummary,
        correlation_id: &str,
    ) -> Result<StorageSummary, ApiError> {
        self.storage_summary_with_mode(request, correlation_id, true)
    }
    /// # Errors
    /// Returns saved observations only. Unknown/stale measurements never trigger I/O.
    pub fn storage_summary_metadata(
        &self,
        request: &GetStorageSummary,
        correlation_id: &str,
    ) -> Result<StorageSummary, ApiError> {
        self.storage_summary_with_mode(request, correlation_id, false)
    }
    fn storage_summary_with_mode(
        &self,
        request: &GetStorageSummary,
        correlation_id: &str,
        refresh: bool,
    ) -> Result<StorageSummary, ApiError> {
        check(request.version, correlation_id)?;
        let resource_request = ListResources {
            version: request.version,
            cask_id: request.cask_id.clone(),
        };
        let items = if refresh {
            self.list_resources(&resource_request, correlation_id)?
        } else {
            self.list_resource_metadata(&resource_request, correlation_id)?
        };
        let mut managed = 0_u64;
        let mut referenced = 0_u64;
        let mut unknown = 0_u64;
        let artifacts = self
            .repository
            .installer_resource_ids(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        for item in items {
            if artifacts.contains(&item.id) {
                continue;
            }
            unknown = unknown.saturating_add(item.unknown_entries);
            if item.availability != "PRESENT" {
                unknown = unknown.saturating_add(1);
                continue;
            }
            if let Some(size) = item.size_bytes {
                if item.ownership == "OWNED" {
                    managed = managed.saturating_add(size);
                } else {
                    referenced = referenced.saturating_add(size);
                }
            } else {
                unknown = unknown.saturating_add(1);
            }
        }
        let entries = self
            .repository
            .application_storage_entries(&request.cask_id)
            .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        let sizes = if refresh {
            let sizes = self
                .storage
                .application_sizes(&entries)
                .map_err(|e| err("STORAGE", e, correlation_id))?;
            self.repository
                .record_application_sizes(&entries, &sizes)
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
            sizes
        } else {
            self.repository
                .cached_application_sizes(&entries)
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?
        };
        let mut owned_applications = 0_u64;
        let mut reference_applications = 0_u64;
        let mut estimated_applications = 0_u64;
        for (entry, size) in entries.iter().zip(sizes) {
            match size {
                ApplicationSize::Missing => {}
                ApplicationSize::Unknown => unknown = unknown.saturating_add(1),
                ApplicationSize::Known { bytes, estimated } => {
                    if entry.owned {
                        owned_applications = owned_applications.saturating_add(bytes);
                    } else {
                        reference_applications = reference_applications.saturating_add(bytes);
                    }
                    estimated_applications += u64::from(estimated);
                }
            }
        }
        Ok(StorageSummary {
            owned_application_bytes: owned_applications,
            reference_application_bytes: reference_applications,
            estimated_applications,
            version: CONTRACT_VERSION,
            managed_logical_bytes: managed,
            referenced_logical_bytes: referenced,
            physical_bytes: None,
            reclaimable_min_bytes: 0,
            reclaimable_max_bytes: managed,
            unknown_entries: unknown,
        })
    }

    fn refresh_availability(
        &self,
        resource: &mut ResourceView,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        let kind = ResourceKind::from_code(&resource.kind).ok_or_else(|| {
            err(
                "PERSISTENCE",
                "Resource kind is invalid".into(),
                correlation_id,
            )
        })?;
        resource.availability = self.storage.availability(&resource.path, kind).into();
        if let Some(measured) = self.storage.measure_resource(&resource.path, kind, false) {
            resource.availability = measured.availability;
            resource.size_bytes = measured.size_bytes;
            resource.unknown_entries = measured.unknown_entries;
            resource.size_measurement = if resource.size_bytes.is_none() {
                "UNKNOWN"
            } else if kind == ResourceKind::Directory {
                "CALCULATED"
            } else {
                "EXACT"
            }
            .into();
            self.refresh_directory_identity(resource, correlation_id)?;
            self.repository
                .record_measurement(resource)
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?;
        }
        Ok(())
    }
    fn refresh_directory_identity(
        &self,
        resource: &mut ResourceView,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        if resource.kind == "DIRECTORY"
            && resource.availability == "PRESENT"
            && let Some(recorded) = self
                .repository
                .recorded_resource_identity(&resource.id)
                .map_err(|e| err("PERSISTENCE", e, correlation_id))?
            && self
                .storage
                .same_directory_object(&resource.path, &recorded)
                == Some(false)
        {
            resource.availability = "CHANGED".into();
        }
        Ok(())
    }
}

fn check(version: u32, correlation_id: &str) -> Result<(), ApiError> {
    if version == CONTRACT_VERSION {
        Ok(())
    } else {
        Err(err(
            "UNSUPPORTED_VERSION",
            format!("Expected contract version {CONTRACT_VERSION}"),
            correlation_id,
        ))
    }
}
fn err(code: &str, message: String, correlation_id: &str) -> ApiError {
    ApiError {
        version: CONTRACT_VERSION,
        code: code.into(),
        message,
        correlation_id: correlation_id.into(),
    }
}
