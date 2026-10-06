//! Persistent installer ownership and durable, inspectable application cleanup.
use crate::installer::InstallerRepository;
use crate::windows_inventory::{
    InstalledInventoryRepository, InstalledInventorySource, ObservedInstallation,
};
use crate::{CaskRepository, CaskService, api_error, check_version};
use casklet_contracts::{
    ApiError, ApproveRemovalPlan, CONTRACT_VERSION, ConfirmInstallerCandidate, GetCask,
    GetRemovalPlan, MsiCleanupItem, MsiCleanupPlan,
};
use sha2::{Digest, Sha256};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub trait MsiRepository {
    /// # Errors
    /// Records whether an exact product existed before the installer ran.
    fn msi_baseline(
        &self,
        operation_id: &str,
        product_code: &str,
        absent: bool,
    ) -> Result<(), String>;
    /// # Errors
    /// Atomically grants ownership only for a verified new successful MSI launch.
    fn claim_msi(
        &self,
        request: &ConfirmInstallerCandidate,
        observed: &ObservedInstallation,
        uninstaller_sha256: Option<&str>,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Lists retained ownership records, never inferred from association.
    fn owned_msi(&self, cask_id: &str) -> Result<Vec<MsiCleanupItem>, String>;
    /// # Errors
    /// Checks every current registered association of this exact product.
    fn msi_dependents(
        &self,
        cask_id: &str,
        installation_id: &str,
        product_code: &str,
    ) -> Result<u64, String>;
    /// # Errors
    /// Saves a preview without executing anything.
    fn save_msi_plan(&self, plan: &MsiCleanupPlan, now: i64) -> Result<(), String>;
    /// # Errors
    fn get_msi_plan(&self, id: &str) -> Result<Option<MsiCleanupPlan>, String>;
    /// # Errors
    fn latest_msi_plan(&self, cask_id: &str) -> Result<Option<MsiCleanupPlan>, String>;
    /// # Errors
    /// Performs a compare-and-set transition; execution takes durable mutation locks.
    fn transition_msi(&self, plan: &MsiCleanupPlan, from: &str, now: i64) -> Result<(), String>;
    /// # Errors
    /// Journals intent or result, retaining ownership until verified disappearance.
    fn msi_step(
        &self,
        plan: &MsiCleanupPlan,
        item: &MsiCleanupItem,
        result: Option<&str>,
        now: i64,
    ) -> Result<(), String>;
    /// # Errors
    /// Releases ownership explicitly while keeping the installed software.
    fn release_msi(&self, cask_id: &str, installation_id: &str, now: i64) -> Result<(), String>;
}

pub trait MsiUninstaller {
    /// # Errors
    /// Executes system msiexec with a validated `ProductCode`; never a registry command.
    fn uninstall_msi(&self, product_code: &str) -> Result<i32, String>;
    /// # Errors
    /// Inspects a registered direct executable; shell commands are unsupported.
    fn inspect_registered_uninstaller(
        &self,
        _item: &ObservedInstallation,
    ) -> Result<String, String> {
        Err("No supported registered uninstaller".into())
    }
    /// # Errors
    /// Runs the exact reviewed executable and waits for its process tree.
    fn uninstall_registered(
        &self,
        _item: &ObservedInstallation,
        _hash: &str,
    ) -> Result<i32, String> {
        Err("No supported registered uninstaller".into())
    }
}

/// MSI `ProductCodes` must be a GUID, never process arguments.
#[must_use]
pub fn product_code(value: &str) -> Option<String> {
    if value.len() != 38 || !value.starts_with('{') || !value.ends_with('}') {
        return None;
    }
    let id = Uuid::parse_str(&value[1..37]).ok()?;
    Some(format!("{{{}}}", id.hyphenated()).to_ascii_uppercase())
}
fn now() -> Result<i64, String> {
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
    )
    .map_err(|e| e.to_string())
}
fn hash(cask_id: &str, items: &[MsiCleanupItem], expires_at_ms: i64) -> String {
    let mut digest = Sha256::new();
    digest.update(expires_at_ms.to_le_bytes());
    for value in std::iter::once(cask_id).chain(items.iter().flat_map(|item| {
        [
            item.installation_id.as_str(),
            &item.name,
            &item.product_code,
            &item.evidence_sha256,
            &item.action,
            item.uninstaller_sha256.as_deref().unwrap_or(""),
        ]
    })) {
        digest.update(value.len().to_le_bytes());
        digest.update(value.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

impl<R, S> CaskService<R, S>
where
    R: CaskRepository + MsiRepository + InstallerRepository + InstalledInventoryRepository,
    S: InstalledInventorySource + MsiUninstaller,
{
    fn msi_error(message: String, correlation: &str) -> ApiError {
        api_error("MSI_CLEANUP", message, correlation)
    }
    /// # Errors
    /// Claims currently verified new MSI/EXE installations, separately from references.
    pub fn claim_managed_msi(
        &self,
        request: &ConfirmInstallerCandidate,
        correlation: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation)?;
        let observed = self
            .storage
            .scan_installed()
            .map_err(|e| Self::msi_error(e, correlation))?
            .into_iter()
            .find(|item| item.installation_id == request.installation_id)
            .ok_or_else(|| Self::msi_error("Installation no longer exists".into(), correlation))?;
        self.repository
            .claim_msi(
                request,
                &observed,
                self.storage
                    .inspect_registered_uninstaller(&observed)
                    .ok()
                    .as_deref(),
                now().map_err(|e| Self::msi_error(e, correlation))?,
            )
            .map_err(|e| Self::msi_error(e, correlation))
    }
    fn msi_items(&self, cask_id: &str) -> Result<Vec<MsiCleanupItem>, String> {
        let observed = self.storage.scan_installed()?;
        let mut items = self.repository.owned_msi(cask_id)?;
        for item in &mut items {
            let matches: Vec<_> = observed
                .iter()
                .filter(|entry| {
                    entry
                        .source_key
                        .rsplit('\\')
                        .next()
                        .is_some_and(|key| key.eq_ignore_ascii_case(&item.product_code))
                        || entry.source_key == item.product_code
                })
                .collect();
            if item.action == "DETACH" {
                continue;
            }
            let verified = matches.len() == 1
                && matches[0].installation_id == item.installation_id
                && matches[0].evidence_sha256 == item.evidence_sha256
                && (product_code(&item.product_code).is_some()
                    || item.uninstaller_sha256.as_ref().is_some_and(|hash| {
                        self.storage
                            .inspect_registered_uninstaller(matches[0])
                            .as_ref()
                            == Ok(hash)
                    }));
            let shared = self.repository.msi_dependents(
                cask_id,
                &item.installation_id,
                &item.product_code,
            )? != 0;
            item.action = if verified && !shared {
                "UNINSTALL"
            } else {
                "PRESERVE"
            }
            .into();
            item.result = if shared {
                Some("Shared installation".into())
            } else if !verified {
                Some("Registration or uninstaller is unavailable, changed or unsupported".into())
            } else {
                None
            };
        }
        Ok(items)
    }
    /// # Errors
    /// Creates or reuses a current preview; never launches an uninstaller.
    pub fn create_msi_cleanup(
        &self,
        request: &GetCask,
        correlation: &str,
    ) -> Result<MsiCleanupPlan, ApiError> {
        self.create_application_cleanup(request, None, correlation)
    }
    fn create_application_cleanup(
        &self,
        request: &GetCask,
        installation_id: Option<&str>,
        correlation: &str,
    ) -> Result<MsiCleanupPlan, ApiError> {
        check_version(request.version, correlation)?;
        let cask = self
            .repository
            .get(&request.id)
            .map_err(|e| Self::msi_error(e, correlation))?
            .ok_or_else(|| Self::msi_error("Cask not found".into(), correlation))?;
        if cask.state != casklet_domain::CaskState::Active {
            return Err(Self::msi_error(
                "Restore this Cask before cleanup".into(),
                correlation,
            ));
        }
        let current = now().map_err(|e| Self::msi_error(e, correlation))?;
        let mut items = self
            .msi_items(&request.id)
            .map_err(|e| Self::msi_error(e, correlation))?;
        if let Some(id) = installation_id {
            items.retain(|item| item.installation_id == id);
            if items.is_empty() {
                return Err(Self::msi_error("Application not found".into(), correlation));
            }
        }
        let expires_at_ms = current + 900_000;
        let plan_hash = hash(&request.id, &items, expires_at_ms);
        if let Some(previous) = self
            .repository
            .latest_msi_plan(&request.id)
            .map_err(|e| Self::msi_error(e, correlation))?
        {
            if matches!(previous.state.as_str(), "EXECUTING" | "PARTIAL") {
                return Err(Self::msi_error(
                    "Inspect and close the interrupted application cleanup first".into(),
                    correlation,
                ));
            }
            if previous.plan_hash == hash(&request.id, &items, previous.expires_at_ms)
                && previous.expires_at_ms > current
                && matches!(previous.state.as_str(), "DRAFT" | "APPROVED")
            {
                return Ok(previous);
            }
        }
        let plan = MsiCleanupPlan {
            version: CONTRACT_VERSION,
            id: Uuid::new_v4().to_string(),
            cask_id: request.id.clone(),
            state: "DRAFT".into(),
            plan_hash,
            expires_at_ms,
            items,
        };
        self.repository
            .save_msi_plan(&plan, current)
            .map_err(|e| Self::msi_error(e, correlation))?;
        Ok(plan)
    }
    /// # Errors
    /// Creates an inspectable plan for one application using the same cleanup queue.
    pub fn create_application_removal(
        &self,
        request: &casklet_contracts::DisassociateInstalledApplication,
        correlation: &str,
    ) -> Result<MsiCleanupPlan, ApiError> {
        self.create_application_cleanup(
            &GetCask {
                version: request.version,
                id: request.cask_id.clone(),
            },
            Some(&request.installation_id),
            correlation,
        )
    }
    /// # Errors
    /// Loads durable state, including interrupted execution.
    pub fn latest_msi_cleanup(
        &self,
        request: &GetCask,
        correlation: &str,
    ) -> Result<Option<MsiCleanupPlan>, ApiError> {
        check_version(request.version, correlation)?;
        self.repository
            .latest_msi_plan(&request.id)
            .map_err(|e| Self::msi_error(e, correlation))
    }
    fn checked_msi_plan(
        &self,
        request: &ApproveRemovalPlan,
        state: &str,
    ) -> Result<MsiCleanupPlan, String> {
        let plan = self
            .repository
            .get_msi_plan(&request.id)?
            .ok_or("Cleanup plan not found")?;
        let cask = self
            .repository
            .get(&plan.cask_id)?
            .ok_or("Cask no longer exists")?;
        if cask.state != casklet_domain::CaskState::Active {
            return Err("Restore this Cask before cleanup".into());
        }
        if plan.state != state
            || plan.plan_hash != request.plan_hash
            || hash(&plan.cask_id, &plan.items, plan.expires_at_ms) != plan.plan_hash
            || plan.expires_at_ms <= now()?
        {
            return Err("Plan changed or expired. Create a new preview".into());
        }
        let mut current = self.msi_items(&plan.cask_id)?;
        current.retain(|item| {
            plan.items
                .iter()
                .any(|selected| selected.installation_id == item.installation_id)
        });
        if hash(&plan.cask_id, &current, plan.expires_at_ms) != plan.plan_hash {
            return Err(
                "Application ownership or registration changed. Create a new preview".into(),
            );
        }
        Ok(plan)
    }
    /// # Errors
    /// Separate explicit approval of the current preview.
    pub fn approve_msi_cleanup(
        &self,
        request: &ApproveRemovalPlan,
        correlation: &str,
    ) -> Result<MsiCleanupPlan, ApiError> {
        check_version(request.version, correlation)?;
        let mut plan = self
            .checked_msi_plan(request, "DRAFT")
            .map_err(|e| Self::msi_error(e, correlation))?;
        plan.state = "APPROVED".into();
        self.repository
            .transition_msi(
                &plan,
                "DRAFT",
                now().map_err(|e| Self::msi_error(e, correlation))?,
            )
            .map_err(|e| Self::msi_error(e, correlation))?;
        Ok(plan)
    }
    fn run_application_uninstaller(&self, item: &MsiCleanupItem) -> Result<i32, String> {
        if product_code(&item.product_code).is_some() {
            self.storage.uninstall_msi(&item.product_code)
        } else {
            let observed = self
                .storage
                .scan_installed()?
                .into_iter()
                .find(|entry| {
                    entry.installation_id == item.installation_id
                        && entry.evidence_sha256 == item.evidence_sha256
                })
                .ok_or("EVIDENCE_CHANGED")?;
            self.storage.uninstall_registered(
                &observed,
                item.uninstaller_sha256
                    .as_deref()
                    .ok_or("UNINSTALLER_UNVERIFIED")?,
            )
        }
    }
    /// # Errors
    /// Executes the approved sequential application queue with durable intent and revalidation.
    pub fn execute_msi_cleanup(
        &self,
        request: &ApproveRemovalPlan,
        correlation: &str,
    ) -> Result<MsiCleanupPlan, ApiError> {
        check_version(request.version, correlation)?;
        let mut plan = self
            .checked_msi_plan(request, "APPROVED")
            .map_err(|e| Self::msi_error(e, correlation))?;
        plan.state = "EXECUTING".into();
        self.repository
            .transition_msi(
                &plan,
                "APPROVED",
                now().map_err(|e| Self::msi_error(e, correlation))?,
            )
            .map_err(|e| Self::msi_error(e, correlation))?;
        // Detach references before launching any owned application uninstaller.
        let mut order: Vec<_> = (0..plan.items.len()).collect();
        order.sort_by_key(|index| plan.items[*index].action != "DETACH");
        let mut failed = false;
        for index in order {
            let mut item = plan.items[index].clone();
            if item.action == "PRESERVE" {
                item.state = "PRESERVED".into();
                plan.items[index] = item;
                continue;
            }
            let mut completed = false;
            let result = catch_unwind(AssertUnwindSafe(|| -> Result<(), String> {
                let current = self.msi_items(&plan.cask_id)?;
                if !current.iter().any(|entry| {
                    entry.installation_id == item.installation_id
                        && entry.action == item.action
                        && entry.product_code == item.product_code
                        && entry.evidence_sha256 == item.evidence_sha256
                }) {
                    return Err("EVIDENCE_CHANGED".into());
                }
                item.state = "RUNNING".into();
                self.repository.msi_step(&plan, &item, None, now()?)?;
                if item.action == "DETACH" {
                    return Ok(());
                }
                let code = self.run_application_uninstaller(&item)?;
                completed = !matches!(code, 3010 | 1641);
                if code != 0 {
                    return Err(format!("UNINSTALL_EXIT_{code}"));
                }
                if self.storage.scan_installed()?.iter().any(|entry| {
                    entry
                        .source_key
                        .rsplit('\\')
                        .next()
                        .is_some_and(|key| key.eq_ignore_ascii_case(&item.product_code))
                        || entry.source_key == item.product_code
                }) {
                    return Err("UNINSTALL_NOT_VERIFIED".into());
                }
                Ok(())
            }))
            .unwrap_or_else(|_| Err("UNINSTALL_INTERRUPTED".into()));
            item.state = if result.is_ok() && item.action == "DETACH" {
                "DETACHED"
            } else if result.is_ok() {
                "SUCCEEDED"
            } else if completed {
                "FAILED"
            } else {
                "UNKNOWN"
            }
            .into();
            item.result = result.as_ref().err().cloned();
            self.repository
                .msi_step(
                    &plan,
                    &item,
                    result.as_ref().err().map(String::as_str),
                    now().map_err(|e| Self::msi_error(e, correlation))?,
                )
                .map_err(|e| Self::msi_error(e, correlation))?;
            plan.items[index] = item;
            if result.is_err() {
                failed = true;
                if !completed && plan.items[index].state == "UNKNOWN" {
                    break;
                }
            }
        }
        plan.state = if failed { "PARTIAL" } else { "COMPLETE" }.into();
        self.repository
            .transition_msi(
                &plan,
                "EXECUTING",
                now().map_err(|e| Self::msi_error(e, correlation))?,
            )
            .map_err(|e| Self::msi_error(e, correlation))?;
        Ok(plan)
    }
    /// # Errors
    /// Closes interrupted work with no replay or inferred uninstall success.
    pub fn close_msi_cleanup(
        &self,
        request: &GetRemovalPlan,
        correlation: &str,
    ) -> Result<MsiCleanupPlan, ApiError> {
        check_version(request.version, correlation)?;
        let mut plan = self
            .repository
            .get_msi_plan(&request.id)
            .map_err(|e| Self::msi_error(e, correlation))?
            .ok_or_else(|| Self::msi_error("Plan not found".into(), correlation))?;
        let from = plan.state.clone();
        if !matches!(from.as_str(), "PARTIAL" | "EXECUTING") {
            return Err(Self::msi_error(
                "Plan is not interrupted".into(),
                correlation,
            ));
        }
        plan.state = "CLOSED_PRESERVED".into();
        self.repository
            .transition_msi(
                &plan,
                &from,
                now().map_err(|e| Self::msi_error(e, correlation))?,
            )
            .map_err(|e| Self::msi_error(e, correlation))?;
        Ok(plan)
    }
    /// # Errors
    /// Explicitly keeps software and releases Cask ownership; does not uninstall.
    pub fn release_managed_msi(
        &self,
        request: &casklet_contracts::DisassociateInstalledApplication,
        correlation: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation)?;
        self.repository
            .release_msi(
                &request.cask_id,
                &request.installation_id,
                now().map_err(|e| Self::msi_error(e, correlation))?,
            )
            .map_err(|e| Self::msi_error(e, correlation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_codes_are_typed_guids_not_commands() {
        assert!(product_code("{12345678-1234-1234-1234-123456789abc}").is_some());
        for value in [
            "/x",
            "{bad}",
            "12345678-1234-1234-1234-123456789abc",
            "{12345678-1234-1234-1234-123456789abc} /quiet",
        ] {
            assert!(product_code(value).is_none());
        }
    }
}
