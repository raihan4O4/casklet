//! Read-only removal planning. Only the core can generate a plan and its items.
use crate::{CaskRepository, CaskService};
use casklet_contracts::{
    ApiError, ApproveRemovalPlan, CONTRACT_VERSION, CloseRemovalRun, CreateRemovalPlan,
    ExecuteRemovalPlan, GetRemovalPlan, ListRemovalPlans, RemovalPlanItemView, RemovalPlanView,
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
    pub explicitly_adopted: bool,
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
    ) -> Result<String, String>;
    /// # Errors
    /// Returns a database error.
    fn get_plan(&self, id: &str) -> Result<Option<RemovalPlanView>, String>;
    /// Lists recent plans, prioritizing interrupted work.
    /// # Errors
    /// Returns a persistence error.
    fn list_plans(&self, cask_id: Option<&str>) -> Result<Vec<RemovalPlanView>, String>;
    /// # Errors
    /// Returns private snapshot preconditions for verification.
    fn plan_items(&self, id: &str) -> Result<Vec<PlannedItem>, String>;
    /// # Errors
    /// Rejects an expired or changed snapshot before persisting approval.
    fn approve_plan(&self, id: &str, hash: &str, now_ms: i64) -> Result<(), String>;
    /// # Errors
    /// Claims the Cask mutation lock and writes the durable run before external work.
    fn begin_execution(
        &self,
        id: &str,
        hash: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Revalidates database preconditions and records the item boundary.
    fn start_item(&self, run_id: &str, item: &PlannedItem, now_ms: i64) -> Result<(), String>;
    /// # Errors
    /// Records an outcome and any association change in one transaction.
    fn finish_item(
        &self,
        run_id: &str,
        item: &PlannedItem,
        state: &str,
        result_code: &str,
        now_ms: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Summarizes the journal, retaining uncertain locks or closing a complete run.
    fn finish_execution(&self, run_id: &str, now_ms: i64) -> Result<(), String>;
    /// # Errors
    /// Retains the lock and marks any started item uncertain after an internal error.
    fn mark_execution_uncertain(&self, run_id: &str, now_ms: i64) -> Result<(), String>;
    /// # Errors
    /// Keeps uncertain data and closes its mutation lock only by explicit preserved resolution.
    fn close_uncertain_preserved(&self, run_id: &str, now_ms: i64) -> Result<String, String>;
}

pub trait RemovalInspection {
    /// Returns current identity evidence only for an exact managed allocation.
    fn verify_managed(&self, cask_root: &str, candidate: &RemovalCandidate) -> Option<String>;
    /// # Errors
    /// Moves only an exactly verified managed file to the platform trash.
    fn trash_verified(
        &self,
        cask_root: &str,
        candidate: &RemovalCandidate,
        expected_identity: &str,
        run_id: &str,
        item_id: &str,
    ) -> Result<(), String>;
    /// Permanently removes only an exactly verified managed file.
    /// # Errors
    /// Returns a storage or verification error; callers retain uncertain outcomes.
    fn delete_permanently_verified(
        &self,
        _cask_root: &str,
        _candidate: &RemovalCandidate,
        _expected_identity: &str,
        _run_id: &str,
        _item_id: &str,
    ) -> Result<(), String> {
        Err("Permanent deletion is unavailable on this storage adapter".into())
    }
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
        if !matches!(
            cask.state,
            casklet_domain::CaskState::Active | casklet_domain::CaskState::PartiallyDestroyed
        ) {
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
                "No file resources to clean up. Remove the Cask if no application dependencies or unfinished work remain; Archive remains available in Settings",
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
                    explicitly_adopted: candidate.explicitly_adopted,
                    created_by_casklet: candidate.provenance == "ADDED_BY_CASKLET",
                    owned: candidate.relationship == "OWNED",
                    delete_policy: candidate.removal_policy == "DELETE_WHEN_UNREFERENCED",
                    has_other_association: candidate.other_associations > 0,
                    present: candidate.availability == "PRESENT",
                    verified: identity.is_some(),
                    broad: identity.is_none(),
                    unknown_entries: candidate.unknown_entries > 0,
                });
                let (action, reason) = if request.permanent && action.code() == "TRASH" {
                    if candidate.kind == "FILE" && candidate.containment == "INTERNAL" {
                        ("DELETE_PERMANENTLY", "EXPLICIT_PERMANENT_FILE")
                    } else if candidate.containment == "INTERNAL" {
                        ("PRESERVE", "PERMANENT_DIRECTORY_UNSUPPORTED")
                    } else {
                        ("PRESERVE", "PERMANENT_RESOURCE_UNSUPPORTED")
                    }
                } else {
                    (action.code(), reason)
                };
                PlannedItem {
                    view: RemovalPlanItemView {
                        id: Uuid::new_v4().to_string(),
                        resource_id: candidate.id,
                        name: candidate.name,
                        path: candidate.path.clone(),
                        action: action.into(),
                        reason_code: reason.into(),
                        expected_size_bytes: candidate.size_bytes,
                        state: "PENDING".into(),
                        result_code: None,
                        recovery_path: None,
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
            permanent: request.permanent,
            cleanup_only: true,
            state: "DRAFT".into(),
            plan_hash: String::new(),
            run_id: None,
            created_at_ms: now,
            expires_at_ms: now.saturating_add(15 * 60 * 1000),
            items: items.iter().map(|item| item.view.clone()).collect(),
        };
        let saved_id = self
            .repository
            .save_plan(&plan, &items, cask.revision)
            .map_err(|_| error("PERSISTENCE", "Could not save removal plan", correlation_id))?;
        plan = self
            .repository
            .get_plan(&saved_id)
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

    /// Returns durable recent cleanup history shared by GUI and CLI.
    /// # Errors
    /// Returns contract validation or persistence errors.
    pub fn list_removal_plans(
        &self,
        request: &ListRemovalPlans,
        correlation_id: &str,
    ) -> Result<Vec<RemovalPlanView>, ApiError> {
        if request.version != CONTRACT_VERSION {
            return Err(error(
                "UNSUPPORTED_VERSION",
                "Contract version mismatch",
                correlation_id,
            ));
        }
        self.repository
            .list_plans(request.cask_id.as_deref())
            .map_err(|_| {
                error(
                    "PERSISTENCE",
                    "Could not load cleanup history",
                    correlation_id,
                )
            })
    }

    /// Approves the exact immutable snapshot for later revalidated execution.
    /// # Errors
    /// Returns validation, stale-plan, or persistence errors.
    pub fn approve_removal_plan(
        &self,
        request: &ApproveRemovalPlan,
        correlation_id: &str,
    ) -> Result<RemovalPlanView, ApiError> {
        if request.version != CONTRACT_VERSION {
            return Err(error(
                "UNSUPPORTED_VERSION",
                "Contract version mismatch",
                correlation_id,
            ));
        }
        let current = self.get_removal_plan(
            &GetRemovalPlan {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )?;
        if current.plan_hash != request.plan_hash {
            return Err(error(
                "PLAN_CHANGED",
                "Removal plan hash differs",
                correlation_id,
            ));
        }
        let cask = self
            .repository
            .get(&current.cask_id)
            .map_err(|_| error("PERSISTENCE", "Could not load Cask", correlation_id))?
            .ok_or_else(|| error("NOT_FOUND", "Cask not found", correlation_id))?;
        let candidates = self
            .repository
            .candidates(&current.cask_id)
            .map_err(|_| error("PERSISTENCE", "Could not load resources", correlation_id))?;
        for item in self
            .repository
            .plan_items(&request.id)
            .map_err(|_| error("PERSISTENCE", "Could not load plan items", correlation_id))?
        {
            if !matches!(item.view.action.as_str(), "TRASH" | "DELETE_PERMANENTLY") {
                continue;
            }
            let Some(candidate) = candidates
                .iter()
                .find(|candidate| candidate.id == item.view.resource_id)
            else {
                return Err(error(
                    "PLAN_CHANGED",
                    "Planned resource is unavailable",
                    correlation_id,
                ));
            };
            if item.identity.is_none()
                || self
                    .storage
                    .verify_managed(&cask.managed_storage_root, candidate)
                    != item.identity
            {
                return Err(error(
                    "PLAN_CHANGED",
                    "Managed item changed after planning",
                    correlation_id,
                ));
            }
        }
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| error("CLOCK", "Clock unavailable", correlation_id))?
                .as_millis(),
        )
        .map_err(|_| error("CLOCK", "Clock unavailable", correlation_id))?;
        self.repository
            .approve_plan(&request.id, &request.plan_hash, now)
            .map_err(|e| error("PLAN_NOT_APPROVABLE", &e, correlation_id))?;
        self.get_removal_plan(
            &GetRemovalPlan {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )
    }

    /// Executes only the approved snapshot; uncertain external outcomes retain the run lock.
    /// # Errors
    /// Returns validation or persistence errors.
    #[expect(
        clippy::too_many_lines,
        reason = "journal boundaries and per-item outcomes are kept visible in one use case"
    )]
    pub fn execute_removal_plan(
        &self,
        request: &ExecuteRemovalPlan,
        correlation_id: &str,
    ) -> Result<RemovalPlanView, ApiError> {
        if request.version != CONTRACT_VERSION {
            return Err(error(
                "UNSUPPORTED_VERSION",
                "Contract version mismatch",
                correlation_id,
            ));
        }
        let plan = self.get_removal_plan(
            &GetRemovalPlan {
                version: request.version,
                id: request.id.clone(),
            },
            correlation_id,
        )?;
        if plan.state != "APPROVED" || plan.plan_hash != request.plan_hash {
            return Err(error(
                "PLAN_NOT_EXECUTABLE",
                "Approve this exact plan first",
                correlation_id,
            ));
        }
        if plan.permanent && !request.confirm_permanent {
            return Err(error(
                "PERMANENT_CONFIRMATION_REQUIRED",
                "Explicit permanent deletion confirmation is required",
                correlation_id,
            ));
        }
        let cask = self
            .repository
            .get(&plan.cask_id)
            .map_err(|_| error("PERSISTENCE", "Could not load Cask", correlation_id))?
            .ok_or_else(|| error("NOT_FOUND", "Cask not found", correlation_id))?;
        let now = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| error("CLOCK", "Clock unavailable", correlation_id))?
                .as_millis(),
        )
        .map_err(|_| error("CLOCK", "Clock unavailable", correlation_id))?;
        let run_id = Uuid::new_v4().to_string();
        self.repository
            .begin_execution(&plan.id, &plan.plan_hash, &run_id, now)
            .map_err(|e| error("PLAN_NOT_EXECUTABLE", &e, correlation_id))?;
        let work =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), ApiError> {
                let items = self
                    .repository
                    .plan_items(&plan.id)
                    .map_err(|e| error("RECOVERY_REQUIRED", &e, correlation_id))?;
                for item in items {
                    let candidates = self
                        .repository
                        .candidates(&plan.cask_id)
                        .map_err(|e| error("RECOVERY_REQUIRED", &e, correlation_id))?;
                    let candidate = candidates
                        .iter()
                        .find(|candidate| candidate.id == item.view.resource_id);
                    let verified =
                        if matches!(item.view.action.as_str(), "TRASH" | "DELETE_PERMANENTLY") {
                            candidate.and_then(|candidate| {
                                self.storage
                                    .verify_managed(&cask.managed_storage_root, candidate)
                            }) == item.identity
                                && item.identity.is_some()
                        } else {
                            true
                        };
                    self.repository
                        .start_item(&run_id, &item, now)
                        .map_err(|e| error("RECOVERY_REQUIRED", &e, correlation_id))?;
                    let (state, result) = match item.view.action.as_str() {
                        "PRESERVE" => ("PRESERVED", "PLAN_PRESERVE"),
                        "DETACH" => ("SUCCEEDED", "DETACHED"),
                        "TRASH" | "DELETE_PERMANENTLY" if !verified => {
                            ("PRESERVED", "REVALIDATION_FAILED")
                        }
                        "TRASH" | "DELETE_PERMANENTLY" => {
                            let Some(candidate) = candidate else {
                                unreachable!("verified target exists")
                            };
                            let Some(identity) = &item.identity else {
                                unreachable!("verified identity exists")
                            };
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    if item.view.action == "TRASH" {
                                        self.storage.trash_verified(
                                            &cask.managed_storage_root,
                                            candidate,
                                            identity,
                                            &run_id,
                                            &item.view.id,
                                        )
                                    } else {
                                        self.storage.delete_permanently_verified(
                                            &cask.managed_storage_root,
                                            candidate,
                                            identity,
                                            &run_id,
                                            &item.view.id,
                                        )
                                    }
                                }));
                            if result.is_ok_and(|result| result.is_ok()) {
                                (
                                    "SUCCEEDED",
                                    if item.view.action == "TRASH" {
                                        "TRASHED"
                                    } else {
                                        "PERMANENTLY_DELETED"
                                    },
                                )
                            } else {
                                (
                                    "UNKNOWN",
                                    if item.view.action == "TRASH" {
                                        "TRASH_OUTCOME_UNCERTAIN"
                                    } else {
                                        "DELETE_OUTCOME_UNCERTAIN"
                                    },
                                )
                            }
                        }
                        _ => ("UNKNOWN", "INVALID_ACTION"),
                    };
                    self.repository
                        .finish_item(&run_id, &item, state, result, now)
                        .map_err(|e| error("RECOVERY_REQUIRED", &e, correlation_id))?;
                    if state == "UNKNOWN" {
                        break;
                    }
                }
                self.repository
                    .finish_execution(&run_id, now)
                    .map_err(|e| error("RECOVERY_REQUIRED", &e, correlation_id))?;
                Ok(())
            }));
        match work {
            Ok(Ok(())) => {}
            Ok(Err(problem)) => {
                let _ = self.repository.mark_execution_uncertain(&run_id, now);
                return Err(problem);
            }
            Err(_) => {
                let _ = self.repository.mark_execution_uncertain(&run_id, now);
                return Err(error(
                    "RECOVERY_REQUIRED",
                    "Cleanup worker interrupted; inspect recovery",
                    correlation_id,
                ));
            }
        }
        self.get_removal_plan(
            &GetRemovalPlan {
                version: request.version,
                id: plan.id,
            },
            correlation_id,
        )
    }

    /// Closes an uncertain run while retaining every unresolved resource.
    /// # Errors
    /// Returns validation or persistence errors.
    pub fn close_uncertain_removal(
        &self,
        request: &CloseRemovalRun,
        correlation_id: &str,
    ) -> Result<RemovalPlanView, ApiError> {
        if request.version != CONTRACT_VERSION {
            return Err(error(
                "UNSUPPORTED_VERSION",
                "Contract version mismatch",
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
        let plan_id = self
            .repository
            .close_uncertain_preserved(&request.run_id, now)
            .map_err(|e| error("RECOVERY_REQUIRED", &e, correlation_id))?;
        self.get_removal_plan(
            &GetRemovalPlan {
                version: request.version,
                id: plan_id,
            },
            correlation_id,
        )
    }
}
