//! SQLite evidence, ownership, plan state, and Windows Installer capability.
use super::SqliteCasks;
use casklet_application::managed_msi::{MsiRepository, product_code};
use casklet_application::windows_inventory::ObservedInstallation;
use casklet_contracts::{
    ConfirmInstallerCandidate, InstallerCandidateView, InstallerInspection, MsiCleanupItem,
    MsiCleanupPlan,
};
use rusqlite::{OptionalExtension, params};

fn queue_projection(tx: &rusqlite::Transaction<'_>, cask_id: &str, now: i64) -> Result<(), String> {
    tx.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![cask_id,now]).map_err(|e|e.to_string())?;
    Ok(())
}
fn read_plan(json: &str) -> Result<MsiCleanupPlan, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

impl MsiRepository for SqliteCasks {
    fn msi_baseline(&self, id: &str, code: &str, absent: bool) -> Result<(), String> {
        let code = product_code(code).ok_or("Invalid MSI ProductCode")?;
        self.connection()?.execute("INSERT INTO installer_msi_baselines(operation_id,product_code,absent_before) VALUES (?1,?2,?3)", params![id,code,absent]).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn claim_msi(
        &self,
        request: &ConfirmInstallerCandidate,
        observed: &ObservedInstallation,
        now: i64,
    ) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let (cask_id, state, exit, inspection, candidates, baseline, absent): (String,String,Option<i32>,String,String,String,bool) = tx.query_row("SELECT o.cask_id,o.state,o.exit_code,a.inspection_json,o.candidates_json,b.product_code,b.absent_before FROM installer_operations o JOIN installer_artifacts a ON a.id=o.artifact_id JOIN installer_msi_baselines b ON b.operation_id=o.id WHERE o.id=?1", [&request.operation_id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).map_err(|_| "This launch has no verified MSI baseline".to_owned())?;
        let inspection: InstallerInspection =
            serde_json::from_str(&inspection).map_err(|e| e.to_string())?;
        let candidates: Vec<InstallerCandidateView> =
            serde_json::from_str(&candidates).map_err(|e| e.to_string())?;
        let code = inspection
            .product_code
            .as_deref()
            .and_then(product_code)
            .ok_or("MSI ProductCode unavailable")?;
        let registration: serde_json::Value =
            serde_json::from_str(&observed.evidence_json).map_err(|e| e.to_string())?;
        let active: bool = tx.query_row("SELECT state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM cask_removals WHERE cask_id=?1) FROM casks WHERE id=?1", [&cask_id], |r| r.get(0)).map_err(|e| e.to_string())?;
        let busy: i64 = tx.query_row("SELECT (SELECT COUNT(*) FROM msi_cleanup_locks)+(SELECT COUNT(*) FROM winget_locks)+(SELECT COUNT(*) FROM installer_locks)+(SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1)+(SELECT COUNT(*) FROM operation_locks WHERE lock_key='cask:'||?1||':mutation')", [&cask_id], |r| r.get(0)).map_err(|e| e.to_string())?;
        let associated: i64 = tx.query_row("SELECT COUNT(*) FROM windows_cask_associations WHERE installation_id=?1 AND removed_at_ms IS NULL", [&observed.installation_id], |r| r.get(0)).map_err(|e|e.to_string())?;
        let existing_product: i64 = tx.query_row("SELECT COUNT(*) FROM managed_msi_installations WHERE product_code=?1 COLLATE NOCASE AND state='OWNED'", [&code], |r|r.get(0)).map_err(|e|e.to_string())?;
        let evidence = casklet_domain::installer::InstallerOwnershipEvidence {
            supported_format: inspection.format == "MSI",
            successful_launch: state == "REVIEW_REQUIRED" && exit == Some(0),
            verified_new_product: absent && baseline == code,
            single_exact_candidate: candidates.len() == 1
                && candidates[0].installation_id == observed.installation_id,
            current_identity: candidates.len() == 1
                && candidates[0].evidence_sha256 == observed.evidence_sha256
                && observed.installation_id == request.installation_id,
            supported_registration: registration["windowsInstaller"].as_u64() == Some(1)
                && observed
                    .source_key
                    .rsplit('\\')
                    .next()
                    .is_some_and(|key| key.eq_ignore_ascii_case(&code)),
            active_cask: active,
            unshared_and_unlocked: busy == 0 && associated == 0 && existing_product == 0,
        };
        if !evidence.permits_ownership() {
            return Err(
                "Only a new, successful, unshared and verified MSI installation can be managed"
                    .into(),
            );
        }
        super::windows_inventory::upsert(&tx, observed, now)?;
        tx.execute("INSERT INTO windows_cask_associations(id,cask_id,installation_id,created_at_ms) VALUES (?1,?2,?3,?4)", params![uuid::Uuid::new_v4().to_string(),cask_id,observed.installation_id,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO managed_msi_installations(installation_id,cask_id,operation_id,product_code,evidence_sha256,display_name,state,claimed_at_ms) VALUES (?1,?2,?3,?4,?5,?6,'OWNED',?7)",params![observed.installation_id,cask_id,request.operation_id,code,observed.evidence_sha256,observed.name,now]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE installer_operations SET state='CONFIRMED_ASSOCIATION',confirmed_installation_id=?2,updated_at_ms=?3,revision=revision+1 WHERE id=?1",params![request.operation_id,observed.installation_id,now]).map_err(|e|e.to_string())?;
        queue_projection(&tx, &cask_id, now)?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn owned_msi(&self, cask_id: &str) -> Result<Vec<MsiCleanupItem>, String> {
        let c = self.connection()?;
        let mut s = c.prepare("SELECT installation_id,display_name,product_code,evidence_sha256 FROM managed_msi_installations WHERE cask_id=?1 AND state='OWNED' ORDER BY installation_id").map_err(|e|e.to_string())?;
        s.query_map([cask_id], |r| {
            Ok(MsiCleanupItem {
                installation_id: r.get(0)?,
                name: r.get(1)?,
                product_code: r.get(2)?,
                evidence_sha256: r.get(3)?,
                action: "PRESERVE".into(),
                state: "PENDING".into(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
    fn msi_dependents(
        &self,
        cask_id: &str,
        installation_id: &str,
        code: &str,
    ) -> Result<u64, String> {
        let count: i64 = self.connection()?.query_row("SELECT (SELECT COUNT(*) FROM windows_cask_associations a JOIN windows_installations w ON w.id=a.installation_id WHERE a.removed_at_ms IS NULL AND a.cask_id!=?1 AND (w.id=?2 OR upper(w.source_key) LIKE '%'||upper(?3)))+(SELECT COUNT(*) FROM managed_msi_installations WHERE product_code=?3 COLLATE NOCASE AND cask_id!=?1 AND state='OWNED')+(SELECT COUNT(*) FROM winget_installations WHERE registry_installation_id=?2 AND state!='REMOVED')",params![cask_id,installation_id,code],|r|r.get(0)).map_err(|e|e.to_string())?;
        u64::try_from(count).map_err(|e| e.to_string())
    }
    fn save_msi_plan(&self, plan: &MsiCleanupPlan, now: i64) -> Result<(), String> {
        self.connection()?.execute("INSERT INTO msi_cleanup_plans(id,cask_id,state,plan_hash,plan_json,updated_at_ms) VALUES (?1,?2,?3,?4,?5,?6)",params![plan.id,plan.cask_id,plan.state,plan.plan_hash,serde_json::to_string(plan).map_err(|e|e.to_string())?,now]).map_err(|e|e.to_string())?;
        Ok(())
    }
    fn get_msi_plan(&self, id: &str) -> Result<Option<MsiCleanupPlan>, String> {
        let c = self.connection()?;
        let record: Option<(String, String, String, String)> = c
            .query_row(
                "SELECT plan_json,cask_id,state,plan_hash FROM msi_cleanup_plans WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let Some((json, cask_id, state, stored_hash)) = record else {
            return Ok(None);
        };
        let mut plan = read_plan(&json)?;
        if plan.id != id
            || plan.cask_id != cask_id
            || plan.state != state
            || plan.plan_hash != stored_hash
            || plan.version != casklet_contracts::CONTRACT_VERSION
        {
            return Err(
                "Stored application plan is inconsistent; preserve and inspect its journal".into(),
            );
        }
        let mut steps = c
            .prepare("SELECT installation_id,state FROM msi_cleanup_steps WHERE plan_id=?1")
            .map_err(|e| e.to_string())?;
        for step in steps
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
        {
            let (id, state) = step.map_err(|e| e.to_string())?;
            if let Some(item) = plan
                .items
                .iter_mut()
                .find(|item| item.installation_id == id)
            {
                item.state = state;
            }
        }
        Ok(Some(plan))
    }
    fn latest_msi_plan(&self, cask_id: &str) -> Result<Option<MsiCleanupPlan>, String> {
        let id: Option<String> = self.connection()?.query_row("SELECT id FROM msi_cleanup_plans WHERE cask_id=?1 ORDER BY CASE WHEN state IN ('EXECUTING','PARTIAL') THEN 0 ELSE 1 END,updated_at_ms DESC,id DESC LIMIT 1",[cask_id],|r|r.get(0)).optional().map_err(|e|e.to_string())?;
        id.map(|id| self.get_msi_plan(&id))
            .transpose()
            .map(Option::flatten)
    }
    fn transition_msi(&self, plan: &MsiCleanupPlan, from: &str, now: i64) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        if plan.state == "CLOSED_PRESERVED" {
            let owner: Option<String> = tx
                .query_row(
                    "SELECT owner_pid FROM msi_cleanup_locks WHERE plan_id=?1",
                    [&plan.id],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .flatten();
            if owner.is_some_and(|owner| super::operations::owner_process_alive(&owner)) {
                return Err("Application cleanup is still running; wait for it to finish".into());
            }
        }
        if plan.state == "EXECUTING" {
            let active: bool = tx.query_row("SELECT state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM cask_removals WHERE cask_id=?1) FROM casks WHERE id=?1",[&plan.cask_id],|r|r.get(0)).map_err(|e|e.to_string())?;
            let busy: i64 = tx.query_row("SELECT (SELECT COUNT(*) FROM installer_locks)+(SELECT COUNT(*) FROM winget_locks)+(SELECT COUNT(*) FROM msi_cleanup_locks)+(SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1)+(SELECT COUNT(*) FROM operation_locks WHERE lock_key='cask:'||?1||':mutation')",[&plan.cask_id],|r|r.get(0)).map_err(|e|e.to_string())?;
            if !active || busy != 0 {
                return Err("Another installation or cleanup is active".into());
            }
            tx.execute(
                "INSERT INTO msi_cleanup_locks(cask_id,plan_id,owner_pid) VALUES (?1,?2,?3)",
                params![plan.cask_id, plan.id, std::process::id().to_string()],
            )
            .map_err(|e| e.to_string())?;
        }
        let changed = tx.execute("UPDATE msi_cleanup_plans SET state=?2,plan_json=?3,updated_at_ms=?4 WHERE id=?1 AND state=?5 AND plan_hash=?6", params![plan.id,plan.state,serde_json::to_string(plan).map_err(|e|e.to_string())?,now,from,plan.plan_hash]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Plan state changed; reload cleanup".into());
        }
        if matches!(plan.state.as_str(), "COMPLETE" | "CLOSED_PRESERVED") {
            tx.execute("DELETE FROM msi_cleanup_locks WHERE plan_id=?1", [&plan.id])
                .map_err(|e| e.to_string())?;
        }
        if plan.state == "PARTIAL" {
            tx.execute(
                "UPDATE msi_cleanup_locks SET owner_pid=NULL WHERE plan_id=?1",
                [&plan.id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    fn msi_step(
        &self,
        plan: &MsiCleanupPlan,
        item: &MsiCleanupItem,
        result: Option<&str>,
        now: i64,
    ) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO msi_cleanup_steps(plan_id,installation_id,state,result_code,at_ms) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(plan_id,installation_id) DO UPDATE SET state=excluded.state,result_code=excluded.result_code,at_ms=excluded.at_ms",params![plan.id,item.installation_id,item.state,result,now]).map_err(|e|e.to_string())?;
        if item.state == "SUCCEEDED" {
            tx.execute("UPDATE managed_msi_installations SET state='REMOVED' WHERE installation_id=?1 AND cask_id=?2 AND state='OWNED'",params![item.installation_id,plan.cask_id]).map_err(|e|e.to_string())?;
            tx.execute("UPDATE windows_cask_associations SET removed_at_ms=?3,revision=revision+1 WHERE installation_id=?1 AND cask_id=?2 AND removed_at_ms IS NULL",params![item.installation_id,plan.cask_id,now]).map_err(|e|e.to_string())?;
        }
        queue_projection(&tx, &plan.cask_id, now)?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn release_msi(&self, cask_id: &str, installation_id: &str, now: i64) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let busy: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM msi_cleanup_locks WHERE cask_id=?1)",
                [cask_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if busy {
            return Err("Inspect and close interrupted application cleanup first".into());
        }
        let changed = tx.execute("UPDATE managed_msi_installations SET state='RELEASED' WHERE installation_id=?1 AND cask_id=?2 AND state='OWNED'",params![installation_id,cask_id]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Managed MSI installation not found".into());
        }
        tx.execute("UPDATE windows_cask_associations SET removed_at_ms=?3,revision=revision+1 WHERE installation_id=?1 AND cask_id=?2 AND removed_at_ms IS NULL",params![installation_id,cask_id,now]).map_err(|e|e.to_string())?;
        queue_projection(&tx, cask_id, now)?;
        tx.commit().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::managed_msi::MsiUninstaller;
    use casklet_application::windows_inventory::{
        InstalledInventoryRepository, InstalledInventorySource,
    };
    use casklet_application::{CaskRepository, CaskService};
    use casklet_contracts::{ApproveRemovalPlan, CONTRACT_VERSION, GetCask, GetRemovalPlan};
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    const CODE: &str = "{12345678-1234-1234-1234-123456789ABC}";
    struct Fake {
        registry: Mutex<Vec<ObservedInstallation>>,
        runs: AtomicUsize,
        fail: bool,
    }
    impl InstalledInventorySource for Fake {
        fn scan_installed(&self) -> Result<Vec<ObservedInstallation>, String> {
            Ok(self.registry.lock().unwrap().clone())
        }
    }
    impl MsiUninstaller for Fake {
        fn uninstall_msi(&self, code: &str) -> Result<i32, String> {
            assert_eq!(code, CODE);
            self.runs.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err("interrupted after launch".into());
            }
            self.registry.lock().unwrap().clear();
            Ok(0)
        }
    }
    struct Fixture {
        service: CaskService<SqliteCasks, Fake>,
        cask_id: String,
        root: std::path::PathBuf,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.root).unwrap();
        }
    }
    impl Fixture {
        fn new(fail: bool) -> Self {
            let root = std::env::temp_dir().join(format!("casklet-msi-{}", uuid::Uuid::new_v4()));
            let repo = SqliteCasks::open(root.join("db.sqlite")).unwrap();
            let cask = casklet_domain::Cask::new(
                "Managed MSI",
                root.join("casks").to_string_lossy().into_owned(),
                1,
            )
            .unwrap();
            repo.create(&cask).unwrap();
            let observed = ObservedInstallation {
                application_id: "app".into(),
                installation_id: "installed".into(),
                source_key: format!("HKCU64\\Uninstall\\{CODE}"),
                scope: "USER".into(),
                name: "Example App".into(),
                publisher: Some("Example".into()),
                display_version: Some("1".into()),
                install_location: None,
                evidence_json: "{\"windowsInstaller\":1}".into(),
                evidence_sha256: "exact-evidence".into(),
            };
            let inspection = InstallerInspection {
                version: CONTRACT_VERSION,
                path: "test.msi".into(),
                format: "MSI".into(),
                sha256: "artifact-hash".into(),
                size_bytes: 1,
                signature_status: "VALID".into(),
                signature_detail: None,
                signer: None,
                product_name: Some("Example App".into()),
                publisher: None,
                product_version: None,
                product_code: Some(CODE.into()),
                warnings: vec![],
            };
            let candidate = InstallerCandidateView {
                installation_id: observed.installation_id.clone(),
                evidence_sha256: observed.evidence_sha256.clone(),
                name: observed.name.clone(),
                publisher: None,
                display_version: None,
                scope: "USER".into(),
                confidence: "HIGH".into(),
                evidence: vec![],
            };
            let c = repo.connection().unwrap();
            c.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_measurement,unknown_entries,created_at_ms) VALUES ('resource','FILE','test.msi','MANAGED','COPIED_BY_CASKLET','PRESENT','UNKNOWN',0,1)",[]).unwrap();
            c.execute("INSERT INTO installer_artifacts(id,cask_id,resource_id,source_path,path,format,sha256,size_bytes,inspection_json,registered_at_ms) VALUES ('artifact',?1,'resource','test.msi','test.msi','MSI','artifact-hash',1,?2,1)",params![cask.id,serde_json::to_string(&inspection).unwrap()]).unwrap();
            c.execute("INSERT INTO installer_operations(id,artifact_id,cask_id,state,exit_code,candidates_json,started_at_ms,updated_at_ms) VALUES ('launch','artifact',?1,'REVIEW_REQUIRED',0,?2,1,1)",params![cask.id,serde_json::to_string(&vec![candidate]).unwrap()]).unwrap();
            repo.msi_baseline("launch", CODE, true).unwrap();
            Self {
                service: CaskService {
                    repository: repo,
                    storage: Fake {
                        registry: Mutex::new(vec![observed]),
                        runs: AtomicUsize::new(0),
                        fail,
                    },
                },
                cask_id: cask.id,
                root,
            }
        }
        fn claim(&self) -> Result<(), casklet_contracts::ApiError> {
            self.service.claim_managed_msi(
                &ConfirmInstallerCandidate {
                    version: CONTRACT_VERSION,
                    operation_id: "launch".into(),
                    installation_id: "installed".into(),
                },
                "test",
            )
        }
        fn preview(&self) -> MsiCleanupPlan {
            self.service
                .create_msi_cleanup(
                    &GetCask {
                        version: CONTRACT_VERSION,
                        id: self.cask_id.clone(),
                    },
                    "test",
                )
                .unwrap()
        }
        fn request(plan: &MsiCleanupPlan) -> ApproveRemovalPlan {
            ApproveRemovalPlan {
                version: CONTRACT_VERSION,
                id: plan.id.clone(),
                plan_hash: plan.plan_hash.clone(),
            }
        }
    }
    #[test]
    fn owned_msi_preview_approval_execution_and_durable_disappearance() {
        let f = Fixture::new(false);
        f.claim().unwrap();
        let plan = f.preview();
        assert_eq!(plan.items[0].action, "UNINSTALL");
        assert_eq!(f.preview().id, plan.id);
        assert_eq!(f.service.storage.runs.load(Ordering::SeqCst), 0);
        assert!(
            f.service
                .execute_msi_cleanup(&Fixture::request(&plan), "test")
                .is_err()
        );
        f.service
            .approve_msi_cleanup(&Fixture::request(&plan), "test")
            .unwrap();
        let result = f
            .service
            .execute_msi_cleanup(&Fixture::request(&plan), "test")
            .unwrap();
        assert_eq!(result.state, "COMPLETE");
        assert_eq!(result.items[0].state, "SUCCEEDED");
        assert_eq!(f.service.storage.runs.load(Ordering::SeqCst), 1);
        assert!(
            f.service
                .repository
                .owned_msi(&f.cask_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            f.service
                .repository
                .list_associated_installed(&f.cask_id)
                .unwrap()
                .is_empty()
        );
        assert!(
            f.service
                .execute_msi_cleanup(&Fixture::request(&plan), "test")
                .is_err()
        );
        let reopened = SqliteCasks::open(f.root.join("db.sqlite")).unwrap();
        assert_eq!(
            reopened.get_msi_plan(&plan.id).unwrap().unwrap().items[0].state,
            "SUCCEEDED"
        );
    }
    #[test]
    fn existing_legacy_uncertain_and_exe_launches_never_gain_ownership() {
        for sql in [
            "DELETE FROM installer_msi_baselines",
            "UPDATE installer_msi_baselines SET absent_before=0",
            "UPDATE installer_operations SET exit_code=3010,state='RECOVERY_REQUIRED'",
            "UPDATE installer_operations SET candidates_json='[]'",
            "UPDATE installer_artifacts SET inspection_json=json_set(inspection_json,'$.format','EXE')",
        ] {
            let f = Fixture::new(false);
            f.service
                .repository
                .connection()
                .unwrap()
                .execute_batch(sql)
                .unwrap();
            assert!(f.claim().is_err());
            assert!(
                f.service
                    .repository
                    .owned_msi(&f.cask_id)
                    .unwrap()
                    .is_empty()
            );
        }
    }
    #[test]
    fn shared_or_changed_products_are_preserved_and_approved_plans_revalidate() {
        let f = Fixture::new(false);
        f.claim().unwrap();
        let plan = f.preview();
        f.service
            .approve_msi_cleanup(&Fixture::request(&plan), "test")
            .unwrap();
        let other = casklet_domain::Cask::new(
            "Other",
            f.root.join("other").to_string_lossy().into_owned(),
            2,
        )
        .unwrap();
        f.service.repository.create(&other).unwrap();
        let observed = f.service.storage.registry.lock().unwrap()[0].clone();
        f.service
            .repository
            .associate_installed(&other.id, &observed, 2)
            .unwrap();
        assert!(
            f.service
                .execute_msi_cleanup(&Fixture::request(&plan), "test")
                .is_err()
        );
        let fresh = f.preview();
        assert_eq!(fresh.items[0].action, "PRESERVE");
        f.service
            .approve_msi_cleanup(&Fixture::request(&fresh), "test")
            .unwrap();
        assert_eq!(
            f.service
                .execute_msi_cleanup(&Fixture::request(&fresh), "test")
                .unwrap()
                .items[0]
                .state,
            "PRESERVED"
        );
        assert_eq!(f.service.storage.runs.load(Ordering::SeqCst), 0);
        assert_eq!(f.service.repository.owned_msi(&f.cask_id).unwrap().len(), 1);
    }
    #[test]
    fn interrupted_uninstall_retains_lock_and_closes_without_replay() {
        let f = Fixture::new(true);
        f.claim().unwrap();
        let plan = f.preview();
        f.service
            .approve_msi_cleanup(&Fixture::request(&plan), "test")
            .unwrap();
        let partial = f
            .service
            .execute_msi_cleanup(&Fixture::request(&plan), "test")
            .unwrap();
        assert_eq!(partial.state, "PARTIAL");
        assert!(
            !f.service
                .repository
                .removal_status(&f.cask_id)
                .unwrap()
                .can_remove
        );
        assert!(
            f.service
                .repository
                .release_msi(&f.cask_id, "installed", 2)
                .is_err()
        );
        let reopened = SqliteCasks::open(f.root.join("db.sqlite")).unwrap();
        assert_eq!(
            reopened.latest_msi_plan(&f.cask_id).unwrap().unwrap().items[0].state,
            "UNKNOWN"
        );
        f.service
            .close_msi_cleanup(
                &GetRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id,
                },
                "test",
            )
            .unwrap();
        assert_eq!(f.service.storage.runs.load(Ordering::SeqCst), 1);
        assert_eq!(f.service.repository.owned_msi(&f.cask_id).unwrap().len(), 1);
        f.service
            .repository
            .release_msi(&f.cask_id, "installed", 3)
            .unwrap();
        assert!(
            f.service
                .repository
                .owned_msi(&f.cask_id)
                .unwrap()
                .is_empty()
        );
        assert_eq!(f.service.storage.registry.lock().unwrap().len(), 1);
    }
    #[test]
    fn tampered_expired_and_changed_evidence_plans_cannot_execute() {
        let f = Fixture::new(false);
        f.claim().unwrap();
        let plan = f.preview();
        let mut request = Fixture::request(&plan);
        request.plan_hash = "wrong".into();
        assert!(f.service.approve_msi_cleanup(&request, "test").is_err());
        f.service.storage.registry.lock().unwrap()[0].evidence_sha256 = "changed".into();
        assert!(
            f.service
                .approve_msi_cleanup(&Fixture::request(&plan), "test")
                .is_err()
        );
        f.service.storage.registry.lock().unwrap()[0].evidence_sha256 = "exact-evidence".into();
        f.service.repository.connection().unwrap().execute("UPDATE msi_cleanup_plans SET plan_json=json_set(plan_json,'$.items[0].productCode','{AAAAAAAA-AAAA-AAAA-AAAA-AAAAAAAAAAAA}') WHERE id=?1",[&plan.id]).unwrap();
        assert!(
            f.service
                .approve_msi_cleanup(&Fixture::request(&plan), "test")
                .is_err()
        );
        f.service.repository.connection().unwrap().execute("UPDATE msi_cleanup_plans SET plan_json=json_set(plan_json,'$.items[0].productCode',?2) WHERE id=?1",params![plan.id,CODE]).unwrap();
        f.service.repository.connection().unwrap().execute("UPDATE msi_cleanup_plans SET plan_json=json_set(plan_json,'$.expiresAtMs',0) WHERE id=?1",[&plan.id]).unwrap();
        assert!(
            f.service
                .approve_msi_cleanup(&Fixture::request(&plan), "test")
                .is_err()
        );
        assert_eq!(f.service.storage.runs.load(Ordering::SeqCst), 0);
    }
}
