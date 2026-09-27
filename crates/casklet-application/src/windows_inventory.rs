//! Read-only Windows discovery and explicit association of existing installations.
use crate::{CaskRepository, CaskService, api_error, check_version};
use crate::{
    projection::{ManifestProjection, ProjectionRepository},
    resource::ResourceRepository,
};
use casklet_contracts::{
    ApiError, AssociateInstalledApplication, CONTRACT_VERSION, DetectedApplicationView,
    DisassociateInstalledApplication, ListAssociatedApplications, RefreshInstalledApplications,
    SearchInstalledApplications,
};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ObservedInstallation {
    pub application_id: String,
    pub installation_id: String,
    pub source_key: String,
    pub scope: String,
    pub name: String,
    pub publisher: Option<String>,
    pub display_version: Option<String>,
    pub install_location: Option<String>,
    pub evidence_json: String,
    pub evidence_sha256: String,
}

pub trait InstalledInventorySource {
    /// # Errors
    /// A partial scan fails rather than implying that unreadable registrations are missing.
    fn scan_installed(&self) -> Result<Vec<ObservedInstallation>, String>;
}

pub trait InstalledInventoryRepository {
    /// # Errors
    /// Reconciles one complete snapshot while preserving missing and changed history.
    fn reconcile_installed(
        &self,
        observed: &[ObservedInstallation],
        now: i64,
    ) -> Result<usize, String>;
    /// # Errors
    /// Records a reference to an exact observed instance; never grants ownership.
    fn associate_installed(
        &self,
        cask_id: &str,
        observed: &ObservedInstallation,
        now: i64,
    ) -> Result<DetectedApplicationView, String>;
    /// # Errors
    /// Retires only the association.
    fn disassociate_installed(
        &self,
        cask_id: &str,
        installation_id: &str,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Returns referenced instances, including missing or changed historical records.
    fn list_associated_installed(
        &self,
        cask_id: &str,
    ) -> Result<Vec<DetectedApplicationView>, String>;
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

#[must_use]
pub fn observed_view(
    item: &ObservedInstallation,
    ambiguous_name: bool,
    cask_id: Option<String>,
) -> DetectedApplicationView {
    DetectedApplicationView {
        version: CONTRACT_VERSION,
        application_id: item.application_id.clone(),
        installation_id: item.installation_id.clone(),
        name: item.name.clone(),
        publisher: item.publisher.clone(),
        display_version: item.display_version.clone(),
        scope: item.scope.clone(),
        source_key: item.source_key.clone(),
        install_location: item.install_location.clone(),
        state: "PRESENT".into(),
        ambiguous_name,
        associated_cask_id: cask_id,
        provenance: "DETECTED_WINDOWS_REGISTRY".into(),
        removal_policy: "DETACH_ONLY".into(),
    }
}

impl<
    R: CaskRepository + InstalledInventoryRepository + ResourceRepository + ProjectionRepository,
    S: InstalledInventorySource + ManifestProjection,
> CaskService<R, S>
{
    /// # Errors
    /// Returns an inventory error if any supported registry view cannot be read.
    pub fn search_installed_applications(
        &self,
        request: &SearchInstalledApplications,
        correlation_id: &str,
    ) -> Result<Vec<DetectedApplicationView>, ApiError> {
        check_version(request.version, correlation_id)?;
        if request.query.chars().count() > 120 {
            return Err(api_error(
                "INVALID_QUERY",
                "Search query is too long".into(),
                correlation_id,
            ));
        }
        let observed = self
            .storage
            .scan_installed()
            .map_err(|e| api_error("INVENTORY", e, correlation_id))?;
        let query = request.query.trim().to_lowercase();
        let mut result: Vec<_> = observed
            .iter()
            .filter(|item| {
                item.name.to_lowercase().contains(&query)
                    || item
                        .publisher
                        .as_ref()
                        .is_some_and(|p| p.to_lowercase().contains(&query))
            })
            .map(|item| {
                let ambiguous = observed
                    .iter()
                    .filter(|other| other.name.eq_ignore_ascii_case(&item.name))
                    .count()
                    > 1;
                observed_view(item, ambiguous, None)
            })
            .collect();
        result.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.source_key.cmp(&b.source_key))
        });
        result.truncate(500);
        Ok(result)
    }

    /// # Errors
    /// A partial scan cannot mark absent registrations missing.
    pub fn refresh_installed_applications(
        &self,
        request: &RefreshInstalledApplications,
        correlation_id: &str,
    ) -> Result<usize, ApiError> {
        check_version(request.version, correlation_id)?;
        let observed = self
            .storage
            .scan_installed()
            .map_err(|e| api_error("INVENTORY", e, correlation_id))?;
        self.repository
            .reconcile_installed(&observed, clock(correlation_id)?)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }

    /// # Errors
    /// The selected ID must still identify one currently observed registry registration.
    pub fn associate_installed_application(
        &self,
        request: &AssociateInstalledApplication,
        correlation_id: &str,
    ) -> Result<DetectedApplicationView, ApiError> {
        check_version(request.version, correlation_id)?;
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
        let observed = self
            .storage
            .scan_installed()
            .map_err(|e| api_error("INVENTORY", e, correlation_id))?;
        let item = observed
            .iter()
            .find(|item| item.installation_id == request.installation_id)
            .ok_or_else(|| {
                api_error(
                    "NOT_FOUND",
                    "Installation is no longer detected".into(),
                    correlation_id,
                )
            })?;
        let view = self
            .repository
            .associate_installed(&request.cask_id, item, clock(correlation_id)?)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(view)
    }

    /// # Errors
    /// Returns a persistence error if the association cannot be retired.
    pub fn disassociate_installed_application(
        &self,
        request: &DisassociateInstalledApplication,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .disassociate_installed(
                &request.cask_id,
                &request.installation_id,
                clock(correlation_id)?,
            )
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(())
    }

    /// # Errors
    /// Returns stored associations, including missing registrations.
    pub fn list_associated_applications(
        &self,
        request: &ListAssociatedApplications,
        correlation_id: &str,
    ) -> Result<Vec<DetectedApplicationView>, ApiError> {
        check_version(request.version, correlation_id)?;
        self.repository
            .list_associated_installed(&request.cask_id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
}
