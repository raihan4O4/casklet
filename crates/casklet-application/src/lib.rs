//! Cask use cases and persistence port.
use casklet_contracts::{ApiError, CONTRACT_VERSION, CaskView, CreateCask, GetCask, ListCasks};
use casklet_domain::{Cask, DomainError};
use std::time::{SystemTime, UNIX_EPOCH};
pub mod diagnostics;
pub mod operation;
pub mod portable;
pub mod projection;
pub mod removal;
pub mod resource;
pub mod windows_inventory;

/// Returns linked lower-layer markers for a dependency-direction smoke test.
#[must_use]
pub const fn linked_boundaries() -> (&'static str, &'static str) {
    (
        casklet_domain::CRATE_BOUNDARY,
        casklet_contracts::CRATE_BOUNDARY,
    )
}

pub trait CaskRepository {
    /// # Errors
    /// Returns a persistence error.
    fn create(&self, cask: &Cask) -> Result<(), String>;
    /// # Errors
    /// Returns a persistence error.
    fn get(&self, id: &str) -> Result<Option<Cask>, String>;
    /// # Errors
    /// Returns a persistence error.
    fn list(&self) -> Result<Vec<Cask>, String>;
}

pub trait ManagedStorage {
    fn root_for(&self, id: &str) -> String;
    /// # Errors
    /// Returns a storage error.
    fn initialize(&self, cask: &Cask) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct CaskService<R, S> {
    pub repository: R,
    pub storage: S,
}

impl<
    R: CaskRepository + resource::ResourceRepository + projection::ProjectionRepository,
    S: ManagedStorage + projection::ManifestProjection,
> CaskService<R, S>
{
    /// # Errors
    /// Returns a typed validation, persistence, or storage error.
    pub fn create(&self, request: &CreateCask, correlation_id: &str) -> Result<CaskView, ApiError> {
        check_version(request.version, correlation_id)?;
        let id_root = self.storage.root_for("pending");
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
                .as_millis(),
        )
        .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?;
        let mut cask =
            Cask::new(&request.name, id_root, now).map_err(|DomainError::InvalidName| {
                api_error(
                    "INVALID_NAME",
                    "Name must contain 1–120 non-control characters".into(),
                    correlation_id,
                )
            })?;
        cask.managed_storage_root = self.storage.root_for(&cask.storage_key);
        self.repository
            .create(&cask)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        self.storage
            .initialize(&cask)
            .map_err(|e| api_error("STORAGE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(view(cask))
    }
    /// # Errors
    /// Returns a typed version, persistence, or not-found error.
    pub fn get(&self, request: &GetCask, correlation_id: &str) -> Result<CaskView, ApiError> {
        check_version(request.version, correlation_id)?;
        let cask = self
            .repository
            .get(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        Ok(view(cask))
    }
    /// # Errors
    /// Returns a typed version or persistence error.
    pub fn list(
        &self,
        request: &ListCasks,
        correlation_id: &str,
    ) -> Result<Vec<CaskView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list()
            .map(|items| items.into_iter().map(view).collect())
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
}

fn check_version(version: u32, correlation_id: &str) -> Result<(), ApiError> {
    if version == CONTRACT_VERSION {
        Ok(())
    } else {
        Err(api_error(
            "UNSUPPORTED_VERSION",
            format!("Expected contract version {CONTRACT_VERSION}"),
            correlation_id,
        ))
    }
}
fn api_error(code: &str, message: String, correlation_id: &str) -> ApiError {
    ApiError {
        version: CONTRACT_VERSION,
        code: code.into(),
        message,
        correlation_id: correlation_id.into(),
    }
}
fn view(cask: Cask) -> CaskView {
    CaskView {
        version: CONTRACT_VERSION,
        id: cask.id,
        name: cask.name,
        state: cask.state.code().into(),
        managed_storage_root: cask.managed_storage_root,
        created_at_ms: cask.created_at_ms,
        updated_at_ms: cask.updated_at_ms,
        revision: cask.revision,
    }
}
