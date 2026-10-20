//! Category selection composes existing immutable plans and durable journals.
use crate::installer::InstallerRepository;
use crate::managed_msi::{MsiRepository, MsiUninstaller};
use crate::removal::{RemovalInspection, RemovalRepository};
use crate::windows_inventory::{InstalledInventoryRepository, InstalledInventorySource};
use crate::{CaskRepository, CaskService, api_error, check_version};
use casklet_contracts::{
    ApiError, ApproveRemovalPlan, CategoryCleanupPlan, CreateCategoryCleanup,
    DisassociateInstalledApplication, ExecuteCategoryCleanup, ExecuteRemovalPlan, GetCask,
    GetRemovalPlan,
};

#[derive(Debug, Clone)]
pub struct WholeCaskIntent {
    pub id: String,
    pub cask_id: String,
    pub resource_plan_id: Option<String>,
    pub application_plan_id: Option<String>,
    pub removed: bool,
    pub warning: Option<String>,
}

pub trait WholeCaskCleanupRepository {
    /// # Errors
    /// Persists the exact component-plan IDs for a whole-Cask removal review.
    fn create_whole_cask_intent(
        &self,
        cask: &str,
        resources: Option<&str>,
        applications: Option<&str>,
    ) -> Result<String, String>;
    /// # Errors
    /// Loads an immutable intent and actual removal status; optionally locates it by file plan.
    fn whole_cask_intent(
        &self,
        cask: &str,
        id: Option<&str>,
        resource_plan: Option<&str>,
    ) -> Result<Option<WholeCaskIntent>, String>;
}

impl<R, S> CaskService<R, S>
where
    R: CaskRepository
        + RemovalRepository
        + MsiRepository
        + InstallerRepository
        + InstalledInventoryRepository
        + WholeCaskCleanupRepository
        + crate::resource::ResourceRepository
        + crate::projection::ProjectionRepository,
    S: RemovalInspection
        + MsiUninstaller
        + InstalledInventorySource
        + crate::ManagedStorage
        + crate::projection::ManifestProjection,
{
    /// Generates one review using existing resource/application plans.
    /// # Errors
    /// Invalid scope, unavailable evidence or persistence failure.
    pub fn create_category_cleanup(
        &self,
        request: &CreateCategoryCleanup,
        correlation: &str,
    ) -> Result<CategoryCleanupPlan, ApiError> {
        check_version(request.scope.version, correlation)?;
        if request.installation_id.is_some()
            && (!request.scope.resource_ids.is_empty()
                || request.scope.category_key.as_deref() != Some("applications"))
        {
            return Err(api_error(
                "INVALID_SCOPE",
                "Choose one application or a category".into(),
                correlation,
            ));
        }
        let resources = if request.installation_id.is_none() {
            match self.create_removal_plan(&request.scope, correlation) {
                Ok(plan) => Some(plan),
                Err(error) if error.code == "EMPTY_PLAN" => None,
                Err(error) => return Err(error),
            }
        } else {
            None
        };
        let applications = if request.scope.resource_ids.is_empty()
            && request
                .scope
                .category_key
                .as_deref()
                .is_none_or(|key| key == "applications")
        {
            let plan = if let Some(id) = &request.installation_id {
                self.create_application_removal(
                    &DisassociateInstalledApplication {
                        version: request.scope.version,
                        cask_id: request.scope.cask_id.clone(),
                        installation_id: id.clone(),
                    },
                    correlation,
                )?
            } else {
                self.create_msi_cleanup(
                    &GetCask {
                        version: request.scope.version,
                        id: request.scope.cask_id.clone(),
                    },
                    correlation,
                )?
            };
            (!plan.items.is_empty()).then_some(plan)
        } else {
            None
        };
        if resources.is_none() && applications.is_none() {
            return Err(api_error(
                "EMPTY_PLAN",
                "No items in this cleanup scope".into(),
                correlation,
            ));
        }
        let cask_removal_id = if request.scope.resource_ids.is_empty()
            && request.scope.category_key.is_none()
            && request.installation_id.is_none()
        {
            Some(
                self.repository
                    .create_whole_cask_intent(
                        &request.scope.cask_id,
                        resources.as_ref().map(|plan| plan.id.as_str()),
                        applications.as_ref().map(|plan| plan.id.as_str()),
                    )
                    .map_err(|e| api_error("PERSISTENCE", e, correlation))?,
            )
        } else {
            None
        };
        Ok(CategoryCleanupPlan {
            cask_removal_id,
            cask_removed: false,
            cask_removal_warning: None,
            version: request.scope.version,
            cask_id: request.scope.cask_id.clone(),
            resources,
            applications,
        })
    }
    /// Reloads only journaled plans; client-provided items never authorize effects.
    /// # Errors
    /// Wrong Cask, changed hash or unavailable journal.
    pub fn get_category_cleanup(
        &self,
        request: &CategoryCleanupPlan,
        correlation: &str,
    ) -> Result<CategoryCleanupPlan, ApiError> {
        check_version(request.version, correlation)?;
        let mut result = request.clone();
        result.cask_removed = false;
        result.cask_removal_warning = None;
        if let Some(id) = &request.cask_removal_id {
            let intent = self
                .repository
                .whole_cask_intent(&request.cask_id, Some(id), None)
                .map_err(|e| api_error("PERSISTENCE", e, correlation))?
                .ok_or_else(|| {
                    api_error(
                        "INVALID_PLAN",
                        "Whole-Cask removal intent is unavailable".into(),
                        correlation,
                    )
                })?;
            if intent.resource_plan_id.as_deref()
                != request.resources.as_ref().map(|plan| plan.id.as_str())
                || intent.application_plan_id.as_deref()
                    != request.applications.as_ref().map(|plan| plan.id.as_str())
            {
                return Err(api_error(
                    "INVALID_PLAN",
                    "Whole-Cask cleanup phases do not match the reviewed intent".into(),
                    correlation,
                ));
            }
            result.cask_removed = intent.removed;
            result.cask_removal_warning = intent.warning;
        }
        if let Some(plan) = &request.resources {
            let current = self.get_removal_plan(
                &GetRemovalPlan {
                    version: request.version,
                    id: plan.id.clone(),
                },
                correlation,
            )?;
            if current.cask_id != request.cask_id || current.plan_hash != plan.plan_hash {
                return Err(api_error(
                    "INVALID_PLAN",
                    "Cleanup snapshot changed".into(),
                    correlation,
                ));
            }
            result.resources = Some(current);
        }
        if let Some(plan) = &request.applications {
            let current = self
                .repository
                .get_msi_plan(&plan.id)
                .map_err(|e| api_error("PERSISTENCE", e, correlation))?
                .ok_or_else(|| {
                    api_error(
                        "NOT_FOUND",
                        "Application queue not found".into(),
                        correlation,
                    )
                })?;
            if current.cask_id != request.cask_id || current.plan_hash != plan.plan_hash {
                return Err(api_error(
                    "INVALID_PLAN",
                    "Application snapshot changed".into(),
                    correlation,
                ));
            }
            result.applications = Some(current);
        }
        if result.resources.is_none() && result.applications.is_none() {
            return Err(api_error(
                "EMPTY_PLAN",
                "No cleanup plans selected".into(),
                correlation,
            ));
        }
        Ok(result)
    }
    /// Reopens persisted whole-Cask cleanup without replaying completed phases.
    /// # Errors
    /// Returns missing component or persistence errors.
    pub fn pending_whole_cask_cleanup(
        &self,
        request: &GetCask,
        resource_plan: Option<&str>,
        correlation: &str,
    ) -> Result<Option<CategoryCleanupPlan>, ApiError> {
        check_version(request.version, correlation)?;
        let Some(intent) = self
            .repository
            .whole_cask_intent(&request.id, None, resource_plan)
            .map_err(|e| api_error("PERSISTENCE", e, correlation))?
        else {
            return Ok(None);
        };
        let resources = intent
            .resource_plan_id
            .as_ref()
            .map(|id| {
                self.get_removal_plan(
                    &GetRemovalPlan {
                        version: casklet_contracts::CONTRACT_VERSION,
                        id: id.clone(),
                    },
                    correlation,
                )
            })
            .transpose()?;
        let applications = intent
            .application_plan_id
            .as_ref()
            .map(|id| {
                self.repository
                    .get_msi_plan(id)
                    .map_err(|e| api_error("PERSISTENCE", e, correlation))?
                    .ok_or_else(|| {
                        api_error(
                            "NOT_FOUND",
                            "Application phase is missing".into(),
                            correlation,
                        )
                    })
            })
            .transpose()?;
        self.get_category_cleanup(
            &CategoryCleanupPlan {
                version: casklet_contracts::CONTRACT_VERSION,
                cask_id: intent.cask_id,
                resources,
                applications,
                cask_removal_id: Some(intent.id),
                cask_removed: false,
                cask_removal_warning: None,
            },
            correlation,
        )
        .map(Some)
    }
    /// Approves the exact reviewed snapshots without external effects.
    /// # Errors
    /// Any existing plan approval/revalidation error.
    pub fn approve_category_cleanup(
        &self,
        request: &CategoryCleanupPlan,
        correlation: &str,
    ) -> Result<CategoryCleanupPlan, ApiError> {
        let mut result = self.get_category_cleanup(request, correlation)?;
        if let Some(plan) = &result.applications
            && plan.state == "DRAFT"
        {
            result.applications = Some(self.approve_msi_cleanup(
                &ApproveRemovalPlan {
                    version: request.version,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                correlation,
            )?);
        }
        if let Some(plan) = &result.resources
            && plan.state == "DRAFT"
        {
            result.resources = Some(self.approve_removal_plan(
                &ApproveRemovalPlan {
                    version: request.version,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                correlation,
            )?);
        }
        Ok(result)
    }
    /// Runs the existing sequential queue before file cleanup. Partial queues stop here.
    /// # Errors
    /// Existing execution/revalidation errors; component journals retain progress.
    pub fn execute_category_cleanup(
        &self,
        request: &ExecuteCategoryCleanup,
        correlation: &str,
    ) -> Result<CategoryCleanupPlan, ApiError> {
        let mut result = self.get_category_cleanup(&request.plan, correlation)?;
        if result.cask_removed {
            return Ok(result);
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| api_error("CLOCK", e.to_string(), correlation))?
            .as_millis();
        if result.resources.as_ref().is_some_and(|plan| {
            plan.state == "APPROVED" && u128::try_from(plan.expires_at_ms).unwrap_or(0) <= now
        }) || result.applications.as_ref().is_some_and(|plan| {
            plan.state == "APPROVED" && u128::try_from(plan.expires_at_ms).unwrap_or(0) <= now
        }) {
            return Err(api_error(
                "PLAN_EXPIRED",
                "Create a fresh cleanup preview".into(),
                correlation,
            ));
        }
        if result.resources.as_ref().is_some_and(|plan| {
            !matches!(plan.state.as_str(), "APPROVED" | "COMPLETE")
                || plan.permanent && !request.confirm_permanent
        }) || result
            .applications
            .as_ref()
            .is_some_and(|plan| !matches!(plan.state.as_str(), "APPROVED" | "COMPLETE"))
        {
            return Err(api_error(
                "REVIEW_REQUIRED",
                "Approve every cleanup phase and confirm permanent deletion before execution"
                    .into(),
                correlation,
            ));
        }
        if let Some(plan) = &result.applications {
            if plan.state != "COMPLETE" {
                result.applications = Some(self.execute_msi_cleanup(
                    &ApproveRemovalPlan {
                        version: request.plan.version,
                        id: plan.id.clone(),
                        plan_hash: plan.plan_hash.clone(),
                    },
                    correlation,
                )?);
            }
            if result
                .applications
                .as_ref()
                .is_some_and(|plan| plan.state != "COMPLETE")
            {
                return Ok(result);
            }
        }
        if let Some(plan) = &result.resources
            && plan.state != "COMPLETE"
        {
            result.resources = Some(self.execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: request.plan.version,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                    confirm_permanent: request.confirm_permanent,
                },
                correlation,
            )?);
        }
        self.finish_whole_cask_cleanup(result, correlation)
    }

    fn finish_whole_cask_cleanup(
        &self,
        mut result: CategoryCleanupPlan,
        correlation: &str,
    ) -> Result<CategoryCleanupPlan, ApiError> {
        if result.cask_removal_id.is_none()
            || result
                .resources
                .as_ref()
                .is_some_and(|plan| plan.state != "COMPLETE")
            || result
                .applications
                .as_ref()
                .is_some_and(|plan| plan.state != "COMPLETE")
        {
            return Ok(result);
        }
        let removal = self
            .get(
                &GetCask {
                    version: result.version,
                    id: result.cask_id.clone(),
                },
                correlation,
            )
            .and_then(|cask| {
                self.remove_cask(
                    &casklet_contracts::RemoveCask {
                        version: result.version,
                        id: cask.id,
                        expected_revision: cask.revision,
                        confirm: true,
                    },
                    correlation,
                )
            });
        match removal {
            Ok(warning) => {
                result.cask_removed = true;
                result.cask_removal_warning = warning;
            }
            Err(mut error) => {
                // Another window may have finalized the same intent after our phase check.
                let current = self
                    .repository
                    .whole_cask_intent(&result.cask_id, result.cask_removal_id.as_deref(), None)
                    .map_err(|e| api_error("PERSISTENCE", e, correlation))?;
                if let Some(current) = current.filter(|intent| intent.removed) {
                    result.cask_removed = true;
                    result.cask_removal_warning = current.warning;
                } else {
                    if error.code == "CASK_NOT_REMOVABLE" {
                        error.message = "Cleanup phases finished, but preserved items or unfinished work still prevent Cask removal. Resolve the remaining items and preview Whole Cask again.".into();
                    }
                    return Err(error);
                }
            }
        }
        Ok(result)
    }
}
