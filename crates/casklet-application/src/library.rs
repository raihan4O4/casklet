//! Presentation queries and metadata commands shared by desktop and CLI.
use crate::portable::PortableRepository;
use crate::projection::{ManifestProjection, ProjectionRepository};
use crate::resource::{ResourceRepository, ResourceStorage};
use crate::windows_inventory::InstalledInventoryRepository;
use crate::winget::WingetRepository;
use crate::{CaskRepository, CaskService, ManagedStorage, api_error, check_version, view};
use casklet_contracts::{
    ApiError, CONTRACT_VERSION, CaskLibraryView, CaskSummaryView, CaskView, GetCask, ListCasks,
    ListResources, UpdateCask,
};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait StorageBrowser {
    fn storage_root(&self) -> String;
    /// # Errors
    /// Opens a registered directory through a typed platform adapter.
    fn open_directory(&self, path: &str) -> Result<(), String>;
}

impl<
    R: CaskRepository + ResourceRepository + ProjectionRepository,
    S: ManagedStorage + ManifestProjection,
> CaskService<R, S>
{
    /// # Errors
    /// Returns current dependency and operation checks for permanent removal.
    pub fn cask_removal_status(
        &self,
        request: &GetCask,
        correlation_id: &str,
    ) -> Result<casklet_contracts::CaskRemovalStatus, ApiError> {
        self.get(request, correlation_id)?;
        self.repository
            .removal_status(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))
    }
    /// # Errors
    /// Requires explicit confirmation and an unchanged, dependency-free Cask.
    pub fn remove_cask(
        &self,
        request: &casklet_contracts::RemoveCask,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        if !request.confirm {
            return Err(api_error(
                "CONFIRMATION_REQUIRED",
                "Confirm permanent removal of this Cask.".into(),
                correlation_id,
            ));
        }
        self.get(
            &GetCask {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )?;
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
                .as_millis(),
        )
        .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?;
        if !self
            .repository
            .remove_empty(&request.id, request.expected_revision, now)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
        {
            return Err(api_error("CASK_NOT_REMOVABLE", "This Cask changed, still has dependencies, or has unfinished work. Refresh and review before removing it.".into(), correlation_id));
        }
        Ok(())
    }
    /// Archives or restores metadata while preserving all associated data.
    /// # Errors
    /// Rejects stale revisions and active or uncertain operations.
    pub fn set_cask_archived(
        &self,
        request: &casklet_contracts::SetCaskArchived,
        correlation_id: &str,
    ) -> Result<CaskView, ApiError> {
        let cask = self.get(
            &GetCask {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )?;
        self.update_cask(
            &UpdateCask {
                version: request.version,
                id: request.id.clone(),
                name: cask.name,
                description: cask.description,
                archived: request.archived,
                expected_revision: request.expected_revision,
            },
            correlation_id,
        )
    }
    /// # Errors
    /// Rejects stale revisions, invalid metadata, and locked Casks.
    pub fn update_cask(
        &self,
        request: &UpdateCask,
        correlation_id: &str,
    ) -> Result<CaskView, ApiError> {
        check_version(request.version, correlation_id)?;
        let mut cask = self
            .repository
            .get(&request.id)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .ok_or_else(|| api_error("NOT_FOUND", "Cask not found".into(), correlation_id))?;
        if cask.revision != request.expected_revision {
            return Err(api_error(
                "CONFLICT",
                "This Cask changed. Refresh before saving.".into(),
                correlation_id,
            ));
        }
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
                .as_millis(),
        )
        .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?;
        cask.update_metadata(&request.name, &request.description, request.archived, now)
            .map_err(|e| {
                api_error(
                    "INVALID_CASK",
                    format!("Invalid Cask metadata or state: {e:?}"),
                    correlation_id,
                )
            })?;
        if !self
            .repository
            .update(&cask, request.expected_revision)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
        {
            return Err(api_error(
                "CONFLICT",
                "This Cask changed or has work in progress. Refresh before saving.".into(),
                correlation_id,
            ));
        }
        let _ = self.retry_manifest_projections(correlation_id);
        Ok(view(cask))
    }
}

impl<
    R: CaskRepository + ResourceRepository + ProjectionRepository,
    S: ManagedStorage + ManifestProjection + StorageBrowser,
> CaskService<R, S>
{
    /// # Errors
    /// Opens only the storage directory resolved from an existing Cask ID.
    pub fn open_cask_storage(
        &self,
        request: &GetCask,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        let cask = self.get(request, correlation_id)?;
        self.storage
            .open_directory(&cask.managed_storage_root)
            .map_err(|e| api_error("OPEN_STORAGE", e, correlation_id))
    }
    /// # Errors
    /// Opens the configured managed-storage root, never a frontend-supplied path.
    pub fn open_storage_root(
        &self,
        request: &ListCasks,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        self.storage
            .open_directory(&self.storage.storage_root())
            .map_err(|e| api_error("OPEN_STORAGE", e, correlation_id))
    }
}

impl<
    R: CaskRepository
        + ResourceRepository
        + ProjectionRepository
        + PortableRepository
        + InstalledInventoryRepository
        + WingetRepository,
    S: ResourceStorage + ManifestProjection + StorageBrowser,
> CaskService<R, S>
{
    /// # Errors
    /// Returns current inventories; missing measurements remain unknown.
    pub fn cask_library(
        &self,
        request: &ListCasks,
        correlation_id: &str,
    ) -> Result<CaskLibraryView, ApiError> {
        check_version(request.version, correlation_id)?;
        let casks = self
            .repository
            .list()
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        let mut result = CaskLibraryView {
            version: CONTRACT_VERSION,
            casks: Vec::new(),
            total_items: 0,
            managed_bytes: 0,
            unknown_entries: 0,
            storage_root: self.storage.storage_root(),
        };
        let mut counted_resources = HashSet::new();
        let mut counted_installations = HashSet::new();
        let mut all_application_resources = HashSet::new();
        for cask in casks {
            let resources = self.list_resources(
                &ListResources {
                    version: request.version,
                    cask_id: cask.id.clone(),
                },
                correlation_id,
            )?;
            let portable = self
                .repository
                .list_portable(&cask.id)
                .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
            let associated = self
                .repository
                .list_associated_installed(&cask.id)
                .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
            let winget = self
                .repository
                .list_winget_installations(&cask.id)
                .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
            let application_resources: HashSet<_> = portable
                .iter()
                .map(|item| item.resource_id.as_str())
                .collect();
            all_application_resources.extend(portable.iter().map(|item| item.resource_id.clone()));
            let installations: HashSet<_> = portable
                .iter()
                .map(|item| item.installation_id.clone())
                .chain(associated.iter().map(|item| item.installation_id.clone()))
                .chain(
                    winget
                        .iter()
                        .map(|item| item.registry_installation_id.clone()),
                )
                .collect();
            let file_count = resources
                .iter()
                .filter(|item| !application_resources.contains(item.id.as_str()))
                .count();
            let mut summary = CaskSummaryView {
                cask: view(cask),
                item_count: file_count + installations.len(),
                file_count,
                application_count: installations.len(),
                managed_bytes: 0,
                referenced_bytes: 0,
                unknown_entries: 0,
            };
            for item in resources {
                let unknown = item.unknown_entries.saturating_add(u64::from(
                    item.availability != "PRESENT" || item.size_bytes.is_none(),
                ));
                summary.unknown_entries = summary.unknown_entries.saturating_add(unknown);
                let size = if item.availability == "PRESENT" {
                    item.size_bytes.unwrap_or(0)
                } else {
                    0
                };
                if item.management == "MANAGED" {
                    summary.managed_bytes = summary.managed_bytes.saturating_add(size);
                } else {
                    summary.referenced_bytes = summary.referenced_bytes.saturating_add(size);
                }
                if counted_resources.insert(item.id.clone()) {
                    result.unknown_entries = result.unknown_entries.saturating_add(unknown);
                    if item.management == "MANAGED" {
                        result.managed_bytes = result.managed_bytes.saturating_add(size);
                    }
                }
            }
            counted_installations.extend(installations);
            result.casks.push(summary);
        }
        result.total_items = counted_resources
            .difference(&all_application_resources)
            .count()
            + counted_installations.len();
        Ok(result)
    }
}
