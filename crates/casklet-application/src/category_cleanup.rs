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

impl<R, S> CaskService<R, S>
where
    R: CaskRepository
        + RemovalRepository
        + MsiRepository
        + InstallerRepository
        + InstalledInventoryRepository,
    S: RemovalInspection + MsiUninstaller + InstalledInventorySource,
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
        Ok(CategoryCleanupPlan {
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
        Ok(result)
    }
}
