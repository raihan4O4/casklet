//! Read-only removal planning. Only the core can generate a plan and its items.
use crate::{CaskRepository, CaskService};
use casklet_contracts::{
    ApiError, CONTRACT_VERSION, CreateRemovalPlan, GetRemovalPlan, RemovalPlanItemView,
    RemovalPlanView,
};
use casklet_domain::removal::{RemovalEvidence, propose};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RemovalCandidate {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: String,
    pub management: String,
    pub containment: String,
    pub provenance: String,
    pub relationship: String,
    pub category_key: String,
    pub removal_policy: String,
    pub availability: String,
    pub size_bytes: Option<u64>,
    pub unknown_entries: u64,
    pub resource_revision: i64,
    pub association_revision: i64,
    pub other_associations: u64,
    pub recorded_identity: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlannedItem {
    pub view: RemovalPlanItemView,
    pub path: String,
    pub kind: String,
    pub identity: Option<String>,
    pub resource_revision: i64,
    pub association_revision: i64,
    pub other_associations: u64,
}

pub trait RemovalRepository {
    /// # Errors
    /// Returns a database error.
    fn candidates(&self, cask_id: &str) -> Result<Vec<RemovalCandidate>, String>;
    /// # Errors
    /// Persists the immutable plan and returns its public view.
    fn save_plan(
        &self,
        plan: &RemovalPlanView,
        items: &[PlannedItem],
        cask_revision: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Returns a database error.
    fn get_plan(&self, id: &str) -> Result<Option<RemovalPlanView>, String>;
}

pub trait RemovalInspection {
    /// Returns current identity evidence only for an exact managed allocation.
    fn verify_managed(&self, cask_root: &str, candidate: &RemovalCandidate) -> Option<String>;
}

fn error(code: &str, message: &str, correlation_id: &str) -> ApiError {
    ApiError {
        version: CONTRACT_VERSION,
        code: code.into(),
        message: message.into(),
        correlation_id: correlation_id.into(),
    }
}

impl<R: CaskRepository + RemovalRepository, S: RemovalInspection> CaskService<R, S> {
    /// # Errors
    /// Returns validation or persistence errors.
    #[expect(
        clippy::too_many_lines,
        reason = "scope validation, conservative policy, and durable preview form one planning use case"
    )]
    pub fn create_removal_plan(
        &self,
        request: &CreateRemovalPlan,
        correlation_id: &str,
    ) -> Result<RemovalPlanView, ApiError> {
        if request.version != CONTRACT_VERSION {
            return Err(error(
                "UNSUPPORTED_VERSION",
                "Contract version mismatch",
                correlation_id,
            ));
        }
        if !request.resource_ids.is_empty() && request.category_key.is_some() {
            return Err(error(
                "INVALID_SCOPE",
                "Choose resources or one category",
                correlation_id,
            ));
        }
        let cask = self
            .repository
            .get(&request.cask_id)
            .map_err(|_| error("PERSISTENCE", "Could not load Cask", correlation_id))?
            .ok_or_else(|| error("NOT_FOUND", "Cask not found", correlation_id))?;
        if cask.state != casklet_domain::CaskState::Active {
            return Err(error(
                "CASK_READ_ONLY",
                "Cask is not active",
                correlation_id,
            ));
        }
        let scope = if request.category_key.is_some() {
            "CATEGORY"
        } else if request.resource_ids.is_empty() {
            "CASK"
        } else {
            "RESOURCES"
        };
        let candidates = self
            .repository
            .candidates(&cask.id)
            .map_err(|_| error("PERSISTENCE", "Could not load resources", correlation_id))?;
        let selected: Vec<_> = candidates
            .into_iter()
            .filter(|candidate| {
                if let Some(category) = &request.category_key {
                    &candidate.category_key == category
                } else {
                    request.resource_ids.is_empty() || request.resource_ids.contains(&candidate.id)
                }
            })
            .collect();
        let unique_ids: std::collections::HashSet<_> = request.resource_ids.iter().collect();
        if !request.resource_ids.is_empty()
            && (selected.len() != unique_ids.len()
                || unique_ids.len() != request.resource_ids.len())
        {
            return Err(error(
                "INVALID_SCOPE",
                "One or more resources are unavailable",
                correlation_id,
            ));
        }
        if selected.is_empty() {
            return Err(error(
                "EMPTY_PLAN",
                "No resources in this scope",
                correlation_id,
            ));
        }
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| error("CLOCK", "Clock unavailable", correlation_id))?
                .as_millis(),
        )
        .map_err(|_| error("CLOCK", "Clock unavailable", correlation_id))?;
        let items: Vec<PlannedItem> = selected
            .into_iter()
            .map(|candidate| {
                let identity = self
                    .storage
                    .verify_managed(&cask.managed_storage_root, &candidate);
                let (action, reason) = propose(RemovalEvidence {
                    managed: candidate.management == "MANAGED",
                    internal: candidate.containment == "INTERNAL",
                    created_by_casklet: candidate.provenance == "ADDED_BY_CASKLET",
                    owned: candidate.relationship == "OWNED",
                    delete_policy: candidate.removal_policy == "DELETE_WHEN_UNREFERENCED",
                    has_other_association: candidate.other_associations > 0,
                    present: candidate.availability == "PRESENT",
                    verified: identity.is_some(),
                    broad: identity.is_none(),
                    unknown_entries: candidate.unknown_entries > 0,
                });
                PlannedItem {
                    view: RemovalPlanItemView {
                        id: Uuid::new_v4().to_string(),
                        resource_id: candidate.id,
                        name: candidate.name,
                        action: action.code().into(),
                        reason_code: reason.into(),
                        expected_size_bytes: candidate.size_bytes,
                    },
                    path: candidate.path,
                    kind: candidate.kind,
                    identity,
                    resource_revision: candidate.resource_revision,
                    association_revision: candidate.association_revision,
                    other_associations: candidate.other_associations,
                }
            })
            .collect();
        let mut plan = RemovalPlanView {
            version: CONTRACT_VERSION,
            id: Uuid::new_v4().to_string(),
            cask_id: cask.id,
            scope: scope.into(),
            category_key: request.category_key.clone(),
            state: "DRAFT".into(),
            plan_hash: String::new(),
            created_at_ms: now,
            expires_at_ms: now.saturating_add(15 * 60 * 1000),
            items: items.iter().map(|item| item.view.clone()).collect(),
        };
        self.repository
            .save_plan(&plan, &items, cask.revision)
            .map_err(|_| error("PERSISTENCE", "Could not save removal plan", correlation_id))?;
        plan = self
            .repository
            .get_plan(&plan.id)
            .map_err(|_| error("PERSISTENCE", "Could not load removal plan", correlation_id))?
            .ok_or_else(|| error("PERSISTENCE", "Saved plan not found", correlation_id))?;
        Ok(plan)
    }

    /// # Errors
    /// Returns validation or persistence errors.
    pub fn get_removal_plan(
        &self,
        request: &GetRemovalPlan,
        correlation_id: &str,
    ) -> Result<RemovalPlanView, ApiError> {
        if request.version != CONTRACT_VERSION {
            return Err(error(
                "UNSUPPORTED_VERSION",
                "Contract version mismatch",
                correlation_id,
            ));
        }
        self.repository
            .get_plan(&request.id)
            .map_err(|_| error("PERSISTENCE", "Could not load removal plan", correlation_id))?
            .ok_or_else(|| error("NOT_FOUND", "Removal plan not found", correlation_id))
    }
}
