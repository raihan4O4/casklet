//! Durable `WinGet` intent, ownership evidence, and separate uninstall approval.
use super::{SqliteCasks, windows_inventory::upsert};
use casklet_application::{
    windows_inventory::ObservedInstallation,
    winget::{WingetOwnedRecord, WingetRepository},
};
use casklet_contracts::{
    CONTRACT_VERSION, WingetInstallationView, WingetOperationView, WingetUninstallPlanView,
};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

fn view(row: &rusqlite::Row<'_>) -> rusqlite::Result<WingetOwnedRecord> {
    let state: String = row.get(6)?;
    Ok(WingetOwnedRecord {
        view: WingetInstallationView {
            version: CONTRACT_VERSION,
            id: row.get(0)?,
            cask_id: row.get(1)?,
            package_id: row.get(2)?,
            package_version: row.get(3)?,
            registry_installation_id: row.get(4)?,
            state: state.clone(),
            provenance: "INSTALLED_BY_CASKLET".into(),
            can_plan_uninstall: state == "INSTALLED",
            warnings: vec!["WinGet installations may be shared outside this Cask.".into()],
        },
        registry_evidence_sha256: row.get(5)?,
        provider_sha256: row.get(7)?,
        revision: row.get(8)?,
    })
}
const OWNED: &str = "SELECT id,cask_id,package_id,package_version,registry_installation_id,registry_evidence_sha256,state,provider_sha256,revision FROM winget_installations";
fn plan_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<WingetUninstallPlanView> {
    Ok(WingetUninstallPlanView {
        version: CONTRACT_VERSION,
        id: row.get(0)?,
        installation_id: row.get(1)?,
        package_id: row.get(2)?,
        package_version: row.get(3)?,
        state: row.get(4)?,
        plan_hash: row.get(5)?,
        expires_at_ms: row.get(6)?,
        warnings: vec![
            "Uninstall may affect other software using this global installation.".into(),
        ],
    })
}

fn assert_no_mutation(tx: &rusqlite::Transaction<'_>, cask_id: &str) -> Result<(), String> {
    let active: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM operation_locks WHERE lock_key=?1",
            [format!("cask:{cask_id}:mutation")],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let removing: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
            [cask_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    if active != 0 || removing != 0 {
        return Err("Cask has another active mutation".into());
    }
    Ok(())
}
fn lock(
    tx: &rusqlite::Transaction<'_>,
    cask_id: &str,
    op_id: &str,
    now: i64,
) -> Result<(), String> {
    assert_no_mutation(tx, cask_id)?;
    tx.execute(
        "INSERT INTO winget_locks(cask_id,operation_id,acquired_at_ms) VALUES (?1,?2,?3)",
        params![cask_id, op_id, now],
    )
    .map_err(|_| "Cask has an active WinGet operation".to_string())?;
    Ok(())
}
fn queue_projection(tx: &rusqlite::Transaction<'_>, cask_id: &str, now: i64) -> Result<(), String> {
    tx.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![cask_id,now]).map_err(|e|e.to_string())?;
    Ok(())
}

impl WingetRepository for SqliteCasks {
    fn begin_winget(
        &self,
        op_id: &str,
        cask_id: &str,
        action: &str,
        id: &str,
        version: &str,
        now: i64,
    ) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let state: String = tx
            .query_row("SELECT state FROM casks WHERE id=?1", [cask_id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if state != "ACTIVE" {
            return Err("Cask is not active".into());
        }
        tx.execute("INSERT INTO winget_operations(id,cask_id,action,package_id,package_version,state,step,started_at_ms,updated_at_ms) VALUES (?1,?2,?3,?4,?5,'RUNNING','PREPARE',?6,?6)",params![op_id,cask_id,action,id,version,now]).map_err(|e|e.to_string())?;
        lock(&tx, cask_id, op_id, now)?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn winget_step(&self, op_id: &str, step: &str, now: i64) -> Result<(), String> {
        self.connection()?.execute("UPDATE winget_operations SET step=?2,updated_at_ms=?3,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![op_id,step,now]).map_err(|e|e.to_string())?;
        Ok(())
    }
    fn winget_uncertain(&self, op_id: &str, code: &str, now: i64) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        tx.execute("UPDATE winget_operations SET state='RECOVERY_REQUIRED',step='RECONCILE',error_code=?2,updated_at_ms=?3,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![op_id,code,now]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE winget_uninstall_plans SET state='RECOVERY_REQUIRED',revision=revision+1 WHERE execution_operation_id=?1 AND state='EXECUTING'",[op_id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn finish_winget_install(
        &self,
        op_id: &str,
        cask_id: &str,
        id: &str,
        version: &str,
        provider_hash: &str,
        observed: &ObservedInstallation,
        now: i64,
    ) -> Result<WingetInstallationView, String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let state:String=tx.query_row("SELECT state FROM winget_operations WHERE id=?1 AND cask_id=?2 AND action='INSTALL' AND package_id=?3 AND package_version=?4",params![op_id,cask_id,id,version],|r|r.get(0)).map_err(|e|e.to_string())?;
        if state != "RUNNING" {
            return Err("Install operation is not running".into());
        }
        let existing: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM windows_installations WHERE id=?1",
                [&observed.installation_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if existing != 0 {
            return Err(
                "Registry identity has prior history; ownership requires manual review".into(),
            );
        }
        upsert(&tx, observed, now)?;
        let installation_id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO winget_installations(id,cask_id,package_id,package_version,registry_installation_id,registry_evidence_sha256,provider_sha256,install_operation_id,state,installed_at_ms,last_verified_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'INSTALLED',?9,?9)",params![installation_id,cask_id,id,version,observed.installation_id,observed.evidence_sha256,provider_hash,op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE winget_operations SET state='SUCCEEDED',step='COMPLETE',updated_at_ms=?2,finished_at_ms=?2,revision=revision+1 WHERE id=?1",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM winget_locks WHERE operation_id=?1", [op_id])
            .map_err(|e| e.to_string())?;
        queue_projection(&tx, cask_id, now)?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(WingetInstallationView {
            version: CONTRACT_VERSION,
            id: installation_id,
            cask_id: cask_id.into(),
            package_id: id.into(),
            package_version: version.into(),
            registry_installation_id: observed.installation_id.clone(),
            state: "INSTALLED".into(),
            provenance: "INSTALLED_BY_CASKLET".into(),
            can_plan_uninstall: true,
            warnings: vec!["WinGet installation may be global or shared.".into()],
        })
    }
    fn get_winget_installation(&self, id: &str) -> Result<Option<WingetOwnedRecord>, String> {
        self.connection()?
            .query_row(&format!("{OWNED} WHERE id=?1"), [id], view)
            .optional()
            .map_err(|e| e.to_string())
    }
    fn list_winget_installations(
        &self,
        cask_id: &str,
    ) -> Result<Vec<WingetInstallationView>, String> {
        let c = self.connection()?;
        let mut s = c
            .prepare(&format!(
                "{OWNED} WHERE cask_id=?1 ORDER BY installed_at_ms,id"
            ))
            .map_err(|e| e.to_string())?;
        s.query_map([cask_id], view)
            .map_err(|e| e.to_string())?
            .map(|r| r.map(|v| v.view))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    fn get_winget_operation(&self, id: &str) -> Result<Option<WingetOperationView>, String> {
        let c = self.connection()?;
        let stale_before = i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_millis(),
        )
        .map_err(|e| e.to_string())?
            - 600_000;
        c.execute("UPDATE winget_operations SET state='RECOVERY_REQUIRED',step='RECONCILE',error_code='STALE_HEARTBEAT',revision=revision+1 WHERE id=?1 AND state='RUNNING' AND updated_at_ms<?2",params![id,stale_before]).map_err(|e|e.to_string())?;
        c.execute("UPDATE winget_uninstall_plans SET state='RECOVERY_REQUIRED',revision=revision+1 WHERE execution_operation_id=?1 AND state='EXECUTING' AND EXISTS(SELECT 1 FROM winget_operations WHERE id=?1 AND state='RECOVERY_REQUIRED')",[id]).map_err(|e|e.to_string())?;
        c.query_row("SELECT id,cask_id,action,package_id,state,step,error_code,cancellation_requested FROM winget_operations WHERE id=?1",[id],|r|Ok(WingetOperationView{version:CONTRACT_VERSION,id:r.get(0)?,cask_id:r.get(1)?,action:r.get(2)?,package_id:r.get(3)?,state:r.get(4)?,step:r.get(5)?,error_code:r.get(6)?,cancellation_requested:r.get::<_,i64>(7)?==1})).optional().map_err(|e|e.to_string())
    }
    fn cancel_winget(&self, id: &str) -> Result<(), String> {
        let changed=self.connection()?.execute("UPDATE winget_operations SET cancellation_requested=1,revision=revision+1 WHERE id=?1 AND state='RUNNING'",[id]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Running WinGet operation not found".into());
        }
        Ok(())
    }
    fn winget_cancelled(&self, id: &str) -> Result<bool, String> {
        let cancelled: i64 = self
            .connection()?
            .query_row(
                "SELECT cancellation_requested FROM winget_operations WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(cancelled != 0)
    }
    fn close_winget_preserved(&self, id: &str, now: i64) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let changed=tx.execute("UPDATE winget_operations SET state='CLOSED_PRESERVED',step='CLOSED',updated_at_ms=?2,finished_at_ms=?2,revision=revision+1 WHERE id=?1 AND state='RECOVERY_REQUIRED'",params![id,now]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Operation is not marked for recovery".into());
        }
        tx.execute("DELETE FROM winget_locks WHERE operation_id=?1", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn winget_blocking_associations(
        &self,
        installation_id: &str,
        registry_id: &str,
    ) -> Result<u64, String> {
        let n:i64=self.connection()?.query_row("SELECT (SELECT COUNT(*) FROM windows_cask_associations WHERE installation_id=?2 AND removed_at_ms IS NULL)+(SELECT COUNT(*) FROM winget_installations WHERE registry_installation_id=?2 AND id<>?1 AND state!='REMOVED')",params![installation_id,registry_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        u64::try_from(n).map_err(|e| e.to_string())
    }
    fn save_winget_plan(
        &self,
        plan: &WingetUninstallPlanView,
        record: &WingetOwnedRecord,
        now: i64,
    ) -> Result<(), String> {
        self.connection()?.execute("INSERT INTO winget_uninstall_plans(id,installation_id,cask_id,package_id,package_version,registry_installation_id,registry_evidence_sha256,provider_sha256,installation_revision,plan_hash,state,created_at_ms,expires_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'DRAFT',?11,?12)",params![plan.id,record.view.id,record.view.cask_id,record.view.package_id,record.view.package_version,record.view.registry_installation_id,record.registry_evidence_sha256,record.provider_sha256,record.revision,plan.plan_hash,now,plan.expires_at_ms]).map_err(|e|e.to_string())?;
        Ok(())
    }
    fn get_winget_plan(&self, id: &str) -> Result<Option<WingetUninstallPlanView>, String> {
        self.connection()?.query_row("SELECT id,installation_id,package_id,package_version,state,plan_hash,expires_at_ms FROM winget_uninstall_plans WHERE id=?1",[id],plan_row).optional().map_err(|e|e.to_string())
    }
    fn approve_winget_plan(&self, id: &str, hash: &str, now: i64) -> Result<(), String> {
        let changed=self.connection()?.execute("UPDATE winget_uninstall_plans AS p SET state='APPROVED',approved_at_ms=?3,revision=revision+1 WHERE p.id=?1 AND p.plan_hash=?2 AND p.state='DRAFT' AND p.expires_at_ms>?3 AND p.installation_revision=(SELECT w.revision FROM winget_installations w WHERE w.id=p.installation_id AND w.state='INSTALLED') AND NOT EXISTS(SELECT 1 FROM windows_cask_associations a WHERE a.installation_id=p.registry_installation_id AND a.removed_at_ms IS NULL)",params![id,hash,now]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Plan expired or installation or associations changed".into());
        }
        Ok(())
    }
    fn begin_winget_uninstall(
        &self,
        plan_id: &str,
        hash: &str,
        op_id: &str,
        now: i64,
    ) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let (cask_id,id,version,registry_id,revision,state,expires):(String,String,String,String,i64,String,i64)=tx.query_row("SELECT cask_id,package_id,package_version,registry_installation_id,installation_revision,state,expires_at_ms FROM winget_uninstall_plans WHERE id=?1 AND plan_hash=?2",params![plan_id,hash],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(|e|e.to_string())?;
        if state != "APPROVED" || expires <= now {
            return Err("Plan is not currently approved".into());
        }
        let current:i64=tx.query_row("SELECT revision FROM winget_installations WHERE id=(SELECT installation_id FROM winget_uninstall_plans WHERE id=?1) AND state='INSTALLED'",[plan_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if current != revision {
            return Err("Installation changed after approval".into());
        }
        let blockers:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM windows_cask_associations WHERE installation_id=?1 AND removed_at_ms IS NULL)+(SELECT COUNT(*) FROM winget_installations WHERE registry_installation_id=?1 AND id<>(SELECT installation_id FROM winget_uninstall_plans WHERE id=?2) AND state!='REMOVED')",params![registry_id,plan_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if blockers != 0 {
            return Err("Installation is shared".into());
        }
        tx.execute("INSERT INTO winget_operations(id,cask_id,action,package_id,package_version,state,step,started_at_ms,updated_at_ms) VALUES (?1,?2,'UNINSTALL',?3,?4,'RUNNING','PREPARE',?5,?5)",params![op_id,cask_id,id,version,now]).map_err(|e|e.to_string())?;
        lock(&tx, &cask_id, op_id, now)?;
        tx.execute("UPDATE winget_uninstall_plans SET state='EXECUTING',execution_operation_id=?2,revision=revision+1 WHERE id=?1",params![plan_id,op_id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn finish_winget_uninstall(&self, plan_id: &str, op_id: &str, now: i64) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let (installation_id,cask_id,registry_id):(String,String,String)=tx.query_row("SELECT installation_id,cask_id,registry_installation_id FROM winget_uninstall_plans WHERE id=?1 AND execution_operation_id=?2 AND state='EXECUTING'",params![plan_id,op_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(|e|e.to_string())?;
        tx.execute("UPDATE winget_installations SET state='REMOVED',last_verified_at_ms=?2,revision=revision+1 WHERE id=?1 AND state='INSTALLED'",params![installation_id,now]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE windows_installations SET state='MISSING',last_verified_at_ms=?2,revision=revision+1 WHERE id=?1",params![registry_id,now]).map_err(|e|e.to_string())?;
        tx.execute(
            "UPDATE winget_uninstall_plans SET state='COMPLETE',revision=revision+1 WHERE id=?1",
            [plan_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("UPDATE winget_operations SET state='SUCCEEDED',step='COMPLETE',updated_at_ms=?2,finished_at_ms=?2,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM winget_locks WHERE operation_id=?1", [op_id])
            .map_err(|e| e.to_string())?;
        queue_projection(&tx, &cask_id, now)?;
        tx.commit().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::{
        CaskRepository, CaskService,
        windows_inventory::InstalledInventorySource,
        winget::{InstalledPackage, WingetProvider},
    };
    use casklet_contracts::{
        ApproveWingetUninstallPlan, CreateWingetUninstallPlan, InstallWingetPackage,
        ReviewWingetPackage, WingetCapabilities,
    };
    use casklet_domain::Cask;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FakeProvider {
        installed: AtomicBool,
        fail: bool,
        ambiguous: bool,
    }
    impl InstalledInventorySource for FakeProvider {
        fn scan_installed(&self) -> Result<Vec<ObservedInstallation>, String> {
            if !self.installed.load(Ordering::SeqCst) {
                return Ok(vec![]);
            }
            let item = ObservedInstallation {
                application_id: "app".into(),
                installation_id: "registry-1".into(),
                source_key: "test".into(),
                scope: "USER".into(),
                name: "Fake App".into(),
                publisher: Some("Test".into()),
                display_version: Some("1.0".into()),
                install_location: None,
                evidence_json: "{}".into(),
                evidence_sha256: "evidence".into(),
            };
            if self.ambiguous {
                let mut other = item.clone();
                other.installation_id = "registry-2".into();
                Ok(vec![item, other])
            } else {
                Ok(vec![item])
            }
        }
    }
    impl WingetProvider for FakeProvider {
        fn capabilities(&self) -> WingetCapabilities {
            WingetCapabilities {
                version: CONTRACT_VERSION,
                search: true,
                install: true,
                uninstall: true,
                reason: None,
            }
        }
        fn search(&self, _: &str) -> Result<String, String> {
            Ok("Fake App".into())
        }
        fn show(&self, _: &str, _: &str) -> Result<(String, String), String> {
            Ok(("Found Fake App [Test.Fake]".into(), "binary-hash".into()))
        }
        fn installed_packages(&self) -> Result<Vec<InstalledPackage>, String> {
            Ok(if self.installed.load(Ordering::SeqCst) {
                vec![InstalledPackage {
                    id: "Test.Fake".into(),
                    version: "1.0".into(),
                }]
            } else {
                vec![]
            })
        }
        fn install(
            &self,
            _: &str,
            _: &str,
            _: bool,
            _: &dyn Fn() -> bool,
            _: &dyn Fn(),
        ) -> Result<(), String> {
            self.installed.store(true, Ordering::SeqCst);
            if self.fail {
                Err("provider failed after mutation".into())
            } else {
                Ok(())
            }
        }
        fn uninstall(
            &self,
            _: &str,
            _: &str,
            _: &dyn Fn() -> bool,
            _: &dyn Fn(),
        ) -> Result<(), String> {
            self.installed.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
    fn setup(fail: bool, ambiguous: bool) -> (CaskService<SqliteCasks, FakeProvider>, String) {
        let root = std::env::temp_dir().join(format!("casklet-winget-test-{}", Uuid::new_v4()));
        let repo = SqliteCasks::open(root.join("state.db")).unwrap();
        let cask = Cask::new("WinGet Test", root.to_string_lossy().into_owned(), 1).unwrap();
        repo.create(&cask).unwrap();
        (
            CaskService {
                repository: repo,
                storage: FakeProvider {
                    installed: AtomicBool::new(false),
                    fail,
                    ambiguous,
                },
            },
            cask.id,
        )
    }
    fn install(
        service: &CaskService<SqliteCasks, FakeProvider>,
        cask_id: &str,
    ) -> (
        Result<WingetInstallationView, casklet_contracts::ApiError>,
        String,
    ) {
        let review = service
            .review_winget_package(
                &ReviewWingetPackage {
                    version: CONTRACT_VERSION,
                    package_id: "Test.Fake".into(),
                    package_version: "1.0".into(),
                },
                "test",
            )
            .unwrap();
        let op = std::sync::Mutex::new(String::new());
        let result = service.install_winget_package(
            &InstallWingetPackage {
                version: CONTRACT_VERSION,
                cask_id: cask_id.into(),
                package_id: "Test.Fake".into(),
                package_version: "1.0".into(),
                review_hash: review.review_hash,
                accept_package_agreements: true,
            },
            "test",
            &|id| *op.lock().unwrap() = id.into(),
        );
        (result, op.into_inner().unwrap())
    }
    #[test]
    fn verified_install_requires_exact_correlated_instance_and_approved_plan() {
        let (service, cask_id) = setup(false, false);
        let (result, op) = install(&service, &cask_id);
        let owned = result.unwrap();
        assert_eq!(
            service
                .repository
                .get_winget_operation(&op)
                .unwrap()
                .unwrap()
                .state,
            "SUCCEEDED"
        );
        let plan = service
            .create_winget_uninstall_plan(
                &CreateWingetUninstallPlan {
                    version: CONTRACT_VERSION,
                    installation_id: owned.id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(plan.state, "DRAFT");
        let approved = service
            .approve_winget_uninstall_plan(
                &ApproveWingetUninstallPlan {
                    version: CONTRACT_VERSION,
                    plan_id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(approved.state, "APPROVED");
        #[cfg(feature = "reviewed-removal")]
        {
            let complete = service
                .execute_winget_uninstall_plan(
                    &casklet_contracts::ExecuteWingetUninstallPlan {
                        version: CONTRACT_VERSION,
                        plan_id: plan.id,
                        plan_hash: plan.plan_hash,
                    },
                    "test",
                    &|_| {},
                )
                .unwrap();
            assert_eq!(complete.state, "COMPLETE");
            assert_eq!(
                service
                    .repository
                    .get_winget_installation(&owned.id)
                    .unwrap()
                    .unwrap()
                    .view
                    .state,
                "REMOVED"
            );
        }
    }
    #[test]
    fn failed_and_ambiguous_installs_never_grant_ownership() {
        for (fail, ambiguous) in [(true, false), (false, true)] {
            let (service, cask_id) = setup(fail, ambiguous);
            let (result, op) = install(&service, &cask_id);
            assert!(result.is_err());
            assert!(
                service
                    .repository
                    .list_winget_installations(&cask_id)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                service
                    .repository
                    .get_winget_operation(&op)
                    .unwrap()
                    .unwrap()
                    .state,
                "RECOVERY_REQUIRED"
            );
            assert!(
                service
                    .repository
                    .begin_winget("new", &cask_id, "INSTALL", "Test.Fake", "1.0", 2)
                    .is_err()
            );
            service.repository.close_winget_preserved(&op, 3).unwrap();
        }
    }
    #[test]
    fn shared_registry_association_blocks_uninstall_plan() {
        let (service, cask_id) = setup(false, false);
        let (result, _) = install(&service, &cask_id);
        let owned = result.unwrap();
        let connection = service.repository.connection().unwrap();
        connection.execute("INSERT INTO windows_cask_associations(id,cask_id,installation_id,created_at_ms) VALUES (?1,?2,?3,1)",params![Uuid::new_v4().to_string(),cask_id,owned.registry_installation_id]).unwrap();
        assert!(
            service
                .create_winget_uninstall_plan(
                    &CreateWingetUninstallPlan {
                        version: CONTRACT_VERSION,
                        installation_id: owned.id
                    },
                    "test"
                )
                .is_err()
        );
    }
}
