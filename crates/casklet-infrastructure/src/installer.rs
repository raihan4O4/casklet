//! SQLite journal for referenced installer artifacts and review-only correlations.
use super::SqliteCasks;
use casklet_application::installer::InstallerRepository;
use casklet_contracts::{
    CONTRACT_VERSION, InstallerArtifactView, InstallerCandidateView, InstallerInspection,
    InstallerOperationView,
};
use rusqlite::{OptionalExtension, params};

fn artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<InstallerArtifactView> {
    let json: String = row.get(3)?;
    let inspection: InstallerInspection = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(InstallerArtifactView {
        version: CONTRACT_VERSION,
        id: row.get(0)?,
        cask_id: row.get(1)?,
        resource_id: row.get(2)?,
        source_path: row.get(5)?,
        inspection,
        registered_at_ms: row.get(4)?,
        warnings: vec!["Managed installer copy; no installation ownership is implied.".into()],
    })
}
fn operation(row: &rusqlite::Row<'_>) -> rusqlite::Result<InstallerOperationView> {
    let json: String = row.get(7)?;
    let candidates: Vec<InstallerCandidateView> = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(InstallerOperationView{version:CONTRACT_VERSION,id:row.get(0)?,artifact_id:row.get(1)?,cask_id:row.get(2)?,state:row.get(3)?,exit_code:row.get(4)?,error_code:row.get(5)?,confirmed_installation_id:row.get(6)?,candidates,started_at_ms:row.get(8)?,warnings:vec!["Candidates are observations, not owned installations. Confirming one only creates a detach-only association.".into()]})
}
const ARTIFACT: &str = "SELECT id,cask_id,resource_id,inspection_json,registered_at_ms,source_path FROM installer_artifacts";
const OPERATION: &str = "SELECT id,artifact_id,cask_id,state,exit_code,error_code,confirmed_installation_id,candidates_json,started_at_ms FROM installer_operations";

fn mark_stale(connection: &rusqlite::Connection, cask_id: Option<&str>) -> Result<(), String> {
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis(),
    )
    .map_err(|e| e.to_string())?;
    connection.execute("UPDATE installer_operations SET state='RECOVERY_REQUIRED',error_code='STALE_HEARTBEAT',updated_at_ms=?1,revision=revision+1 WHERE state='RUNNING' AND updated_at_ms<?2 AND (?3 IS NULL OR cask_id=?3)",params![now,now-600_000,cask_id]).map_err(|e|e.to_string())?;
    Ok(())
}

impl InstallerRepository for SqliteCasks {
    fn register_installer(&self, view: &InstallerArtifactView) -> Result<(), String> {
        self.connection()?.execute("INSERT INTO installer_artifacts(id,cask_id,resource_id,source_path,path,format,sha256,size_bytes,inspection_json,registered_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",params![view.id,view.cask_id,view.resource_id,view.source_path,view.inspection.path,view.inspection.format,view.inspection.sha256,i64::try_from(view.inspection.size_bytes).map_err(|e|e.to_string())?,serde_json::to_string(&view.inspection).map_err(|e|e.to_string())?,view.registered_at_ms]).map_err(|e|e.to_string())?;
        Ok(())
    }
    fn get_installer(&self, id: &str) -> Result<Option<InstallerArtifactView>, String> {
        self.connection()?
            .query_row(&format!("{ARTIFACT} WHERE id=?1"), [id], artifact)
            .optional()
            .map_err(|e| e.to_string())
    }
    fn list_installers(&self, cask_id: &str) -> Result<Vec<InstallerArtifactView>, String> {
        let c = self.connection()?;
        let mut s = c
            .prepare(&format!(
                "{ARTIFACT} WHERE cask_id=?1 ORDER BY registered_at_ms,id"
            ))
            .map_err(|e| e.to_string())?;
        s.query_map([cask_id], artifact)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    fn begin_installer(
        &self,
        id: &str,
        artifact_id: &str,
        cask_id: &str,
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
        let busy:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM installer_locks)+(SELECT COUNT(*) FROM winget_locks)+(SELECT COUNT(*) FROM operation_locks WHERE lock_key=?1)+(SELECT COUNT(*) FROM removal_locks WHERE cask_id=?2)",params![format!("cask:{cask_id}:mutation"),cask_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if busy != 0 {
            return Err("Another provider or Cask mutation is active".into());
        }
        tx.execute("INSERT INTO installer_operations(id,artifact_id,cask_id,state,started_at_ms,updated_at_ms) VALUES (?1,?2,?3,'RUNNING',?4,?4)",params![id,artifact_id,cask_id,now]).map_err(|e|e.to_string())?;
        tx.execute(
            "INSERT INTO installer_locks(cask_id,operation_id,acquired_at_ms) VALUES (?1,?2,?3)",
            params![cask_id, id, now],
        )
        .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn installer_heartbeat(&self, id: &str, now: i64) -> Result<(), String> {
        self.connection()?
            .execute(
                "UPDATE installer_operations SET updated_at_ms=?2 WHERE id=?1 AND state='RUNNING'",
                params![id, now],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn cancel_installer(&self, id: &str) -> Result<(), String> {
        let n=self.connection()?.execute("UPDATE installer_operations SET cancellation_requested=1,revision=revision+1 WHERE id=?1 AND state='RUNNING'",[id]).map_err(|e|e.to_string())?;
        if n != 1 {
            return Err("Running installer operation not found".into());
        }
        Ok(())
    }
    fn installer_cancelled(&self, id: &str) -> Result<bool, String> {
        let n: i64 = self
            .connection()?
            .query_row(
                "SELECT cancellation_requested FROM installer_operations WHERE id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(n != 0)
    }
    fn finish_installer(
        &self,
        id: &str,
        state: &str,
        exit_code: Option<i32>,
        error_code: Option<&str>,
        candidates: &[InstallerCandidateView],
        now: i64,
    ) -> Result<(), String> {
        if !matches!(state, "REVIEW_REQUIRED" | "RECOVERY_REQUIRED") {
            return Err("Invalid installer outcome".into());
        }
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let n=tx.execute("UPDATE installer_operations SET state=?2,exit_code=?3,error_code=?4,candidates_json=?5,updated_at_ms=?6,finished_at_ms=?6,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![id,state,exit_code,error_code,serde_json::to_string(candidates).map_err(|e|e.to_string())?,now]).map_err(|e|e.to_string())?;
        if n != 1 {
            return Err("Installer operation no longer running".into());
        }
        if state == "REVIEW_REQUIRED" {
            tx.execute("DELETE FROM installer_locks WHERE operation_id=?1", [id])
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
    fn get_installer_operation(&self, id: &str) -> Result<Option<InstallerOperationView>, String> {
        let c = self.connection()?;
        mark_stale(&c, None)?;
        c.query_row(&format!("{OPERATION} WHERE id=?1"), [id], operation)
            .optional()
            .map_err(|e| e.to_string())
    }
    fn list_installer_operations(
        &self,
        cask_id: &str,
    ) -> Result<Vec<InstallerOperationView>, String> {
        let c = self.connection()?;
        mark_stale(&c, Some(cask_id))?;
        let mut s = c
            .prepare(&format!(
                "{OPERATION} WHERE cask_id=?1 ORDER BY started_at_ms DESC,id DESC LIMIT 100"
            ))
            .map_err(|e| e.to_string())?;
        s.query_map([cask_id], operation)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    fn close_installer_preserved(&self, id: &str, now: i64) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let n=tx.execute("UPDATE installer_operations SET state='CLOSED_PRESERVED',updated_at_ms=?2,revision=revision+1 WHERE id=?1 AND state IN ('REVIEW_REQUIRED','RECOVERY_REQUIRED')",params![id,now]).map_err(|e|e.to_string())?;
        if n != 1 {
            return Err("Reviewable installer operation not found".into());
        }
        tx.execute("DELETE FROM installer_locks WHERE operation_id=?1", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
    fn confirm_installer_candidate(
        &self,
        id: &str,
        installation_id: &str,
        evidence_sha256: &str,
        now: i64,
    ) -> Result<(), String> {
        let mut c = self.connection()?;
        let tx = c.transaction().map_err(|e| e.to_string())?;
        let (state, json): (String, String) = tx
            .query_row(
                "SELECT state,candidates_json FROM installer_operations WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if !matches!(state.as_str(), "REVIEW_REQUIRED" | "RECOVERY_REQUIRED") {
            return Err("Operation is not awaiting review".into());
        }
        let candidates: Vec<InstallerCandidateView> =
            serde_json::from_str(&json).map_err(|e| e.to_string())?;
        if !candidates
            .iter()
            .any(|v| v.installation_id == installation_id && v.evidence_sha256 == evidence_sha256)
        {
            return Err("Candidate evidence changed".into());
        }
        tx.execute("UPDATE installer_operations SET state='CONFIRMED_ASSOCIATION',confirmed_installation_id=?2,updated_at_ms=?3,revision=revision+1 WHERE id=?1",params![id,installation_id,now]).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM installer_locks WHERE operation_id=?1", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalStorage;
    use casklet_application::{
        CaskRepository, CaskService, ManagedStorage,
        installer::{InstallerOutcome, InstallerSource},
        projection::ManifestProjection,
        resource::{IntakePaths, Observation, ResourceStorage},
        windows_inventory::{
            InstalledInventoryRepository, InstalledInventorySource, ObservedInstallation,
        },
        winget::WingetRepository,
    };
    use casklet_contracts::{
        CloseInstallerOperation, ConfirmInstallerCandidate, GetInstallerOperation,
        InspectInstallerArtifact, LaunchInstallerArtifact, RegisterInstallerArtifact,
    };
    use casklet_domain::{
        Cask,
        resource::{IntakeMode, Resource, ResourceKind},
    };
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    #[derive(Clone, Copy)]
    enum Mode {
        Success,
        ExeSuccess,
        Ambiguous,
        Failure,
        Cancelled,
        TimedOut,
        Reboot,
    }
    struct FakeStorage {
        local: LocalStorage,
        mode: Mode,
        registry: Mutex<Vec<ObservedInstallation>>,
        runs: AtomicUsize,
    }
    fn observed(id: &str) -> ObservedInstallation {
        ObservedInstallation {
            application_id: format!("app-{id}"),
            installation_id: id.into(),
            source_key: format!("HKCU64\\Uninstall\\{id}"),
            scope: "USER".into(),
            name: "Example App".into(),
            publisher: Some("Example Ltd".into()),
            display_version: Some("1.0".into()),
            install_location: None,
            evidence_json: format!("{{\"id\":\"{id}\"}}"),
            evidence_sha256: format!("evidence-{id}"),
        }
    }
    impl InstalledInventorySource for FakeStorage {
        fn scan_installed(&self) -> Result<Vec<ObservedInstallation>, String> {
            Ok(self.registry.lock().unwrap().clone())
        }
    }
    impl InstallerSource for FakeStorage {
        fn inspect_installer(&self, path: &str) -> Result<InstallerInspection, String> {
            let canonical = PathBuf::from(path)
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let bytes = fs::read(&canonical).map_err(|e| e.to_string())?;
            Ok(InstallerInspection {
                version: CONTRACT_VERSION,
                path: canonical.to_string_lossy().into_owned(),
                format: if canonical
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("msi"))
                {
                    "MSI"
                } else {
                    "EXE"
                }
                .into(),
                sha256: format!("{:x}", Sha256::digest(&bytes)),
                size_bytes: bytes.len() as u64,
                signature_status: "VALID".into(),
                signature_detail: Some("Signature valid".into()),
                signer: Some("CN=Example Ltd".into()),
                product_name: Some("Example App".into()),
                publisher: Some("Example Ltd".into()),
                product_version: Some("1.0".into()),
                product_code: None,
                warnings: vec!["Signature is evidence, not safety".into()],
            })
        }
        fn run_installer(
            &self,
            _: &InstallerInspection,
            _: bool,
            _: &dyn Fn() -> bool,
            _: &dyn Fn(),
        ) -> Result<InstallerOutcome, String> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            let mut registry = self.registry.lock().unwrap();
            *registry = match self.mode {
                Mode::Ambiguous => vec![observed("one"), observed("two")],
                _ => vec![observed("one")],
            };
            match self.mode {
                Mode::Success | Mode::ExeSuccess | Mode::Ambiguous => {
                    Ok(InstallerOutcome::Exited(0))
                }
                Mode::Failure => Err("fake process failure".into()),
                Mode::Cancelled => Ok(InstallerOutcome::Cancelled),
                Mode::TimedOut => Ok(InstallerOutcome::TimedOut),
                Mode::Reboot => Ok(InstallerOutcome::Exited(3010)),
            }
        }
    }
    impl ResourceStorage for FakeStorage {
        fn inspect(
            &self,
            source: &str,
            progress: &dyn Fn(u64, u64),
        ) -> Result<Observation, String> {
            self.local.inspect(source, progress)
        }
        fn availability(&self, path: &str, kind: ResourceKind) -> &'static str {
            self.local.availability(path, kind)
        }
        fn allocate(&self, cask: &Cask, id: &str, name: &str) -> IntakePaths {
            self.local.allocate(cask, id, name)
        }
        fn materialize(
            &self,
            observed: &Observation,
            paths: &IntakePaths,
            mode: IntakeMode,
            progress: &dyn Fn(u64, u64),
            cancel: &dyn Fn() -> bool,
        ) -> Result<Option<String>, String> {
            self.local
                .materialize(observed, paths, mode, progress, cancel)
        }
        fn finish_staged(
            &self,
            cask: &Cask,
            resource: &Resource,
            paths: &IntakePaths,
            observed: &Observation,
        ) -> Result<Option<String>, String> {
            self.local.finish_staged(cask, resource, paths, observed)
        }
    }
    impl ManifestProjection for FakeStorage {
        fn project(
            &self,
            cask: &Cask,
            resources: &[casklet_contracts::ResourceView],
            categories: &[casklet_contracts::CategoryView],
            apps: &[casklet_contracts::DetectedApplicationView],
        ) -> Result<(), String> {
            self.local.project(cask, resources, categories, apps)
        }
    }
    fn setup(mode: Mode) -> (CaskService<SqliteCasks, FakeStorage>, String, String) {
        let root = std::env::temp_dir().join(format!("casklet-m9-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join(if matches!(mode, Mode::Success | Mode::Ambiguous) {
            "setup.msi"
        } else {
            "setup.exe"
        });
        fs::write(&source, b"fake installer bytes").unwrap();
        let local = LocalStorage::new(root.join("casks"));
        let cask = Cask::new(
            "Installer Test",
            root.join("cask").to_string_lossy().into_owned(),
            1,
        )
        .unwrap();
        local.initialize(&cask).unwrap();
        let repo = SqliteCasks::open(root.join("state.db")).unwrap();
        repo.create(&cask).unwrap();
        (
            CaskService {
                repository: repo,
                storage: FakeStorage {
                    local,
                    mode,
                    registry: Mutex::new(vec![]),
                    runs: AtomicUsize::new(0),
                },
            },
            cask.id,
            source.to_string_lossy().into_owned(),
        )
    }
    fn register(
        service: &CaskService<SqliteCasks, FakeStorage>,
        cask_id: &str,
        path: &str,
    ) -> InstallerArtifactView {
        let preview = service
            .inspect_installer_artifact(
                &InspectInstallerArtifact {
                    version: CONTRACT_VERSION,
                    path: path.into(),
                },
                "test",
            )
            .unwrap();
        service
            .register_installer_artifact(
                &RegisterInstallerArtifact {
                    version: CONTRACT_VERSION,
                    cask_id: cask_id.into(),
                    path: path.into(),
                    expected_sha256: preview.sha256,
                },
                "test",
            )
            .unwrap()
    }
    fn launch(
        service: &CaskService<SqliteCasks, FakeStorage>,
        item: &InstallerArtifactView,
    ) -> InstallerOperationView {
        service
            .launch_installer_artifact(
                &LaunchInstallerArtifact {
                    version: CONTRACT_VERSION,
                    artifact_id: item.id.clone(),
                    expected_sha256: item.inspection.sha256.clone(),
                    accept_execution_risk: true,
                    allow_elevation: false,
                },
                "test",
                &|_| {},
            )
            .unwrap()
    }
    #[test]
    fn registration_copies_bytes_without_execution_and_review_only_associates() {
        let (service, cask_id, source) = setup(Mode::Success);
        let item = register(&service, &cask_id, &source);
        assert_eq!(service.storage.runs.load(Ordering::SeqCst), 0);
        assert!(PathBuf::from(&item.inspection.path).exists());
        assert_ne!(item.inspection.path, source);
        let op = launch(&service, &item);
        assert_eq!(op.state, "REVIEW_REQUIRED");
        assert_eq!(op.candidates.len(), 1);
        assert_eq!(op.candidates[0].confidence, "MEDIUM");
        assert!(
            service
                .repository
                .list_associated_installed(&cask_id)
                .unwrap()
                .is_empty()
        );
        let confirmed = service
            .confirm_installer_candidate(
                &ConfirmInstallerCandidate {
                    version: CONTRACT_VERSION,
                    operation_id: op.id,
                    installation_id: op.candidates[0].installation_id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(confirmed.state, "CONFIRMED_ASSOCIATION");
        let associated = service
            .repository
            .list_associated_installed(&cask_id)
            .unwrap();
        assert_eq!(associated.len(), 1);
        assert_eq!(associated[0].removal_policy, "DETACH_ONLY");
    }
    #[test]
    fn ambiguous_and_uncertain_outcomes_never_claim_installation() {
        for mode in [
            Mode::Ambiguous,
            Mode::ExeSuccess,
            Mode::Failure,
            Mode::Cancelled,
            Mode::TimedOut,
            Mode::Reboot,
        ] {
            let (service, cask_id, source) = setup(mode);
            let item = register(&service, &cask_id, &source);
            let op = launch(&service, &item);
            assert!(
                service
                    .repository
                    .list_associated_installed(&cask_id)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                op.candidates.len(),
                if matches!(mode, Mode::Ambiguous) {
                    2
                } else {
                    1
                }
            );
            assert_eq!(
                op.state,
                if matches!(mode, Mode::Ambiguous) {
                    "REVIEW_REQUIRED"
                } else {
                    "RECOVERY_REQUIRED"
                }
            );
            if op.state == "RECOVERY_REQUIRED" {
                assert!(
                    service
                        .repository
                        .begin_installer("again", &item.id, &cask_id, 2)
                        .is_err()
                );
                assert!(
                    service
                        .repository
                        .begin_winget(
                            "winget-while-installer-uncertain",
                            &cask_id,
                            "INSTALL",
                            "Example.App",
                            "1.0",
                            2
                        )
                        .is_err()
                );
            }
            service
                .close_installer_operation(
                    &CloseInstallerOperation {
                        version: CONTRACT_VERSION,
                        id: op.id.clone(),
                    },
                    "test",
                )
                .unwrap();
            assert_eq!(
                service
                    .get_installer_operation(
                        &GetInstallerOperation {
                            version: CONTRACT_VERSION,
                            id: op.id
                        },
                        "test"
                    )
                    .unwrap()
                    .state,
                "CLOSED_PRESERVED"
            );
        }
    }
    #[test]
    fn changed_candidate_evidence_blocks_confirmation() {
        let (service, cask_id, source) = setup(Mode::Success);
        let item = register(&service, &cask_id, &source);
        let op = launch(&service, &item);
        service.storage.registry.lock().unwrap()[0].evidence_sha256 = "changed".into();
        assert!(
            service
                .confirm_installer_candidate(
                    &ConfirmInstallerCandidate {
                        version: CONTRACT_VERSION,
                        operation_id: op.id,
                        installation_id: "one".into()
                    },
                    "test"
                )
                .is_err()
        );
        assert!(
            service
                .repository
                .list_associated_installed(&cask_id)
                .unwrap()
                .is_empty()
        );
    }
}
