//! SQLite and managed-storage adapters.
use casklet_application::{CaskRepository, CaskService, ManagedStorage};
use casklet_domain::{Cask, CaskState};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::Duration};
mod diagnostics;
mod operations;
mod projection;
mod removal;
mod resource_storage;
mod resources;

pub const CRATE_BOUNDARY: &str = "infrastructure";

#[derive(Debug, Clone)]
pub struct SqliteCasks {
    db_path: PathBuf,
}

impl SqliteCasks {
    /// # Errors
    /// Returns an I/O or migration error.
    pub fn open(db_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let this = Self { db_path };
        let mut connection = this.connection()?;
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        match version {
            0 => {
                let transaction = connection.transaction().map_err(|e| e.to_string())?;
                transaction
                    .execute_batch(include_str!("../../../migrations/0001_casks.sql"))
                    .map_err(|e| e.to_string())?;
                transaction
                    .execute_batch("PRAGMA user_version=1")
                    .map_err(|e| e.to_string())?;
                transaction.commit().map_err(|e| e.to_string())?;
            }
            1..=5 => {}
            _ => return Err(format!("unsupported database schema version: {version}")),
        }
        if version == 1 || version == 0 {
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            transaction
                .execute_batch(include_str!("../../../migrations/0002_resources.sql"))
                .map_err(|e| e.to_string())?;
            transaction
                .execute_batch("PRAGMA user_version=2")
                .map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
        }
        if version <= 2 {
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            transaction
                .execute_batch(include_str!("../../../migrations/0003_operations.sql"))
                .map_err(|e| e.to_string())?;
            transaction
                .execute_batch("PRAGMA user_version=3")
                .map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
        }
        if version <= 3 {
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            transaction
                .execute_batch(include_str!("../../../migrations/0004_removal_plans.sql"))
                .map_err(|e| e.to_string())?;
            transaction
                .execute_batch("PRAGMA user_version=4")
                .map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
        }
        if version <= 4 {
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            transaction
                .execute_batch(include_str!(
                    "../../../migrations/0005_removal_execution.sql"
                ))
                .map_err(|e| e.to_string())?;
            transaction
                .execute_batch("PRAGMA user_version=5")
                .map_err(|e| e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
        }
        Ok(this)
    }
    fn connection(&self) -> Result<Connection, String> {
        let c = Connection::open(&self.db_path).map_err(|e| e.to_string())?;
        c.busy_timeout(Duration::from_secs(5))
            .map_err(|e| e.to_string())?;
        c.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;",
        )
        .map_err(|e| e.to_string())?;
        Ok(c)
    }
}

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Cask> {
    let state_code: String = row.get(7)?;
    let state = CaskState::from_code(&state_code).ok_or_else(|| {
        rusqlite::Error::InvalidColumnType(7, "state".into(), rusqlite::types::Type::Text)
    })?;
    Ok(Cask {
        id: row.get(0)?,
        name: row.get(1)?,
        storage_key: row.get(2)?,
        managed_storage_root: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
        revision: row.get(6)?,
        state,
    })
}
const SELECT: &str = "SELECT id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state FROM casks";

impl CaskRepository for SqliteCasks {
    fn create(&self, cask: &Cask) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO casks (id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![cask.id,cask.name,cask.storage_key,cask.managed_storage_root,cask.created_at_ms,cask.updated_at_ms,cask.revision,cask.state.code()]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,0,'PENDING',?2)",
            params![cask.id,cask.created_at_ms]).map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn get(&self, id: &str) -> Result<Option<Cask>, String> {
        use rusqlite::OptionalExtension;
        self.connection()?
            .query_row(&format!("{SELECT} WHERE id=?1"), [id], row)
            .optional()
            .map_err(|e| e.to_string())
    }
    fn list(&self) -> Result<Vec<Cask>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(&format!("{SELECT} ORDER BY created_at_ms,id"))
            .map_err(|e| e.to_string())?;
        statement
            .query_map([], row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone)]
pub struct LocalStorage {
    root: PathBuf,
}
impl LocalStorage {
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}
impl ManagedStorage for LocalStorage {
    fn root_for(&self, id: &str) -> String {
        self.root.join(id).to_string_lossy().into_owned()
    }
    fn initialize(&self, cask: &Cask) -> Result<(), String> {
        let root = PathBuf::from(&cask.managed_storage_root);
        fs::create_dir_all(root.join("content")).map_err(|e| e.to_string())?;
        let metadata = root.join(".casklet");
        fs::create_dir_all(&metadata).map_err(|e| e.to_string())?;
        let manifest = serde_json::json!({"schemaVersion":1,"cask":{"id":cask.id,"name":cask.name,"state":cask.state.code(),"createdAtMs":cask.created_at_ms},"resources":[],"applications":[]});
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        let temp = metadata.join("manifest.json.tmp");
        let hash = format!("{:x}", Sha256::digest(&bytes));
        fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        fs::rename(temp, metadata.join("manifest.json")).map_err(|e| e.to_string())?;
        fs::write(metadata.join("manifest.sha256"), hash).map_err(|e| e.to_string())
    }
}

/// # Errors
/// Returns an I/O or migration error when development state cannot open.
pub fn development_service() -> Result<CaskService<SqliteCasks, LocalStorage>, String> {
    let root = std::env::var_os("CASKLET_DEV_ROOT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.casklet-dev"),
        PathBuf::from,
    );
    let repository = SqliteCasks::open(root.join("db/casklet.db"))?;
    repository.detect_abandoned_operations()?;
    repository.detect_abandoned_removals()?;
    let service = CaskService {
        repository,
        storage: LocalStorage::new(root.join("casks")),
    };
    let _ = service.retry_manifest_projections("startup");
    Ok(service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::resource::{IntakePaths, ResourceRepository, ResourceStorage};
    use casklet_contracts::{
        AddMode, AddResource, CONTRACT_VERSION, CategorizeResource, CreateCask, CreateCategory,
        GetCask, GetResource, GetStorageSummary, ListCasks, ListResources,
    };
    use casklet_contracts::{
        ApproveRemovalPlan, CloseRemovalRun, CreateRemovalPlan, ExecuteRemovalPlan, GetRemovalPlan,
    };
    use casklet_contracts::{
        CancelOperation, CreateDiagnosticsBundle, ListOperations, PreserveOperation,
        RecoverOperation,
    };
    use casklet_domain::resource::IntakeMode;
    use casklet_domain::resource::Resource;
    #[test]
    fn migration_and_restart() {
        let root = std::env::temp_dir().join(format!("casklet-m1-{}", uuid::Uuid::new_v4()));
        let db = root.join("db/state.db");
        let storage = root.join("casks");
        let service = CaskService {
            repository: SqliteCasks::open(db.clone()).unwrap(),
            storage: LocalStorage::new(storage.clone()),
        };
        let first = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Research".into(),
                },
                "test",
            )
            .unwrap();
        let second = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Research".into(),
                },
                "test",
            )
            .unwrap();
        assert_ne!(first.id, second.id);
        drop(service);
        let reopened = CaskService {
            repository: SqliteCasks::open(db).unwrap(),
            storage: LocalStorage::new(storage),
        };
        let schema_version: i64 = reopened
            .repository
            .connection()
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(schema_version, 5);
        assert_eq!(
            reopened
                .list(
                    &ListCasks {
                        version: CONTRACT_VERSION
                    },
                    "test"
                )
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            reopened
                .get(
                    &GetCask {
                        version: CONTRACT_VERSION,
                        id: first.id.clone()
                    },
                    "test"
                )
                .unwrap()
                .id,
            first.id
        );
        assert!(
            PathBuf::from(first.managed_storage_root)
                .join(".casklet/manifest.json")
                .exists()
        );
        assert!(
            PathBuf::from(second.managed_storage_root)
                .join(".casklet/manifest.sha256")
                .exists()
        );
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "integration fixture covers persistence, copy, reference, and immutable preview"
    )]
    fn removal_preview_tracks_verified_files_and_preserves_replacements() {
        let root = std::env::temp_dir().join(format!("casklet-m4-plan-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Plan test".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"managed").unwrap();
        let managed = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let reference = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Reference,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        assert_eq!(plan.items.len(), 2);
        assert_eq!(
            plan.items
                .iter()
                .find(|i| i.resource_id == managed.id)
                .unwrap()
                .action,
            "TRASH"
        );
        assert_eq!(
            plan.items
                .iter()
                .find(|i| i.resource_id == reference.id)
                .unwrap()
                .action,
            "DETACH"
        );
        assert_eq!(plan.plan_hash.len(), 64);
        let loaded = service
            .get_removal_plan(
                &GetRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(loaded.plan_hash, plan.plan_hash);
        assert_eq!(loaded.items.len(), 2);
        assert_eq!(
            service
                .approve_removal_plan(
                    &ApproveRemovalPlan {
                        version: CONTRACT_VERSION,
                        id: plan.id.clone(),
                        plan_hash: plan.plan_hash.clone(),
                    },
                    "test"
                )
                .unwrap()
                .state,
            "APPROVED"
        );
        assert!(
            service
                .approve_removal_plan(
                    &ApproveRemovalPlan {
                        version: CONTRACT_VERSION,
                        id: plan.id.clone(),
                        plan_hash: "wrong".into(),
                    },
                    "test"
                )
                .is_err()
        );
        assert!(std::path::Path::new(&managed.path).exists());
        assert!(source.exists());
        let pending = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![managed.id.clone()],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        fs::remove_file(&managed.path).unwrap();
        fs::write(&managed.path, b"managed").unwrap();
        assert_eq!(
            service
                .approve_removal_plan(
                    &ApproveRemovalPlan {
                        version: CONTRACT_VERSION,
                        id: pending.id,
                        plan_hash: pending.plan_hash,
                    },
                    "test"
                )
                .unwrap_err()
                .code,
            "PLAN_CHANGED"
        );
        let replaced = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![managed.id.clone()],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        assert_eq!(replaced.items[0].action, "PRESERVE");
        assert_eq!(
            loaded
                .items
                .iter()
                .find(|i| i.resource_id == managed.id)
                .unwrap()
                .action,
            "TRASH"
        );
        assert_eq!(
            service
                .create_removal_plan(
                    &CreateRemovalPlan {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id,
                        resource_ids: vec![managed.id.clone(), managed.id],
                        category_key: None
                    },
                    "test"
                )
                .unwrap_err()
                .code,
            "INVALID_SCOPE"
        );
        let connection = service.repository.connection().unwrap();
        connection
            .execute(
                "UPDATE removal_plan_items SET expected_path='tampered' WHERE id=?1",
                [&plan.items[0].id],
            )
            .unwrap();
        drop(connection);
        assert!(
            service
                .get_removal_plan(
                    &GetRemovalPlan {
                        version: CONTRACT_VERSION,
                        id: plan.id,
                    },
                    "test"
                )
                .is_err()
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "integration fixture covers journal uncertainty and mutation lock after restart"
    )]
    fn interrupted_removal_run_keeps_its_lock_and_unknown_item() {
        let root = std::env::temp_dir().join(format!("casklet-m4-run-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Recovery".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"keep").unwrap();
        let managed = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![managed.id.clone()],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        let run_id = uuid::Uuid::new_v4().to_string();
        let connection = service.repository.connection().unwrap();
        connection
            .execute(
                "UPDATE removal_plans SET state='EXECUTING' WHERE id=?1",
                [&plan.id],
            )
            .unwrap();
        connection.execute("INSERT INTO removal_runs(id,plan_id,cask_id,state,owner_pid,started_at_ms,updated_at_ms) VALUES (?1,?2,?3,'RUNNING','4294967295',1,1)", params![run_id,plan.id,cask.id]).unwrap();
        connection
            .execute(
                "INSERT INTO removal_locks(cask_id,run_id,acquired_at_ms) VALUES (?1,?2,1)",
                params![cask.id, run_id],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE removal_plan_items SET run_id=?1,started_at_ms=1 WHERE id=?2",
                params![run_id, plan.items[0].id],
            )
            .unwrap();
        drop(connection);
        service
            .repository
            .detect_abandoned_removals_with(|_| false)
            .unwrap();
        let connection = service.repository.connection().unwrap();
        let state: String = connection
            .query_row(
                "SELECT state FROM removal_runs WHERE id=?1",
                [&run_id],
                |row| row.get(0),
            )
            .unwrap();
        let item_state: String = connection
            .query_row(
                "SELECT state FROM removal_plan_items WHERE id=?1",
                [&plan.items[0].id],
                |row| row.get(0),
            )
            .unwrap();
        let lock_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&cask.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state, "RECOVERY_REQUIRED");
        assert_eq!(item_state, "UNKNOWN");
        assert_eq!(lock_count, 1);
        assert!(std::path::Path::new(&managed.path).exists());
        drop(connection);
        assert!(
            service
                .add_resource(
                    &AddResource {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id,
                        source_path: source.to_string_lossy().into_owned(),
                        mode: AddMode::Copy,
                        category_key: "other".into(),
                    },
                    "test",
                    &|_, _| {}
                )
                .is_err()
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "integration fixture checks approval, per-item journal, and lifecycle"
    )]
    fn approved_cleanup_uses_journal_and_preserves_reference_source() {
        struct FakeTrash {
            storage: LocalStorage,
            stash: PathBuf,
        }
        impl casklet_application::removal::RemovalInspection for FakeTrash {
            fn verify_managed(
                &self,
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
            ) -> Option<String> {
                casklet_application::removal::RemovalInspection::verify_managed(
                    &self.storage,
                    root,
                    candidate,
                )
            }
            fn trash_verified(
                &self,
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
                identity: &str,
                _run_id: &str,
                _item_id: &str,
            ) -> Result<(), String> {
                if self.verify_managed(root, candidate).as_deref() != Some(identity) {
                    return Err("identity changed".into());
                }
                fs::rename(&candidate.path, &self.stash).map_err(|e| e.to_string())
            }
        }
        let root =
            std::env::temp_dir().join(format!("casklet-m4-execute-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        let storage_root = root.join("casks");
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(storage_root.clone()),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Cleanup".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"retain this source").unwrap();
        let managed = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let _reference = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Reference,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        let stash = root.join("fake-recycle.txt");
        let executor = CaskService {
            repository: repository.clone(),
            storage: FakeTrash {
                storage: LocalStorage::new(storage_root),
                stash: stash.clone(),
            },
        };
        let result = executor
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id,
                    plan_hash: plan.plan_hash,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.state, "COMPLETE");
        assert!(result.items.iter().all(|item| item.state == "SUCCEEDED"));
        assert!(!std::path::Path::new(&managed.path).exists());
        assert!(stash.exists());
        assert!(source.exists());
        assert_eq!(
            repository.get(&cask.id).unwrap().unwrap().state,
            CaskState::Destroyed
        );
        drop(executor);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "integration fixture checks path replacement after approval"
    )]
    fn execution_preserves_replaced_file_after_approval() {
        struct NoTrash(LocalStorage);
        impl casklet_application::removal::RemovalInspection for NoTrash {
            fn verify_managed(
                &self,
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
            ) -> Option<String> {
                casklet_application::removal::RemovalInspection::verify_managed(
                    &self.0, root, candidate,
                )
            }
            fn trash_verified(
                &self,
                _root: &str,
                _candidate: &casklet_application::removal::RemovalCandidate,
                _identity: &str,
                _run_id: &str,
                _item_id: &str,
            ) -> Result<(), String> {
                panic!("changed file must never reach trash")
            }
        }
        let root = std::env::temp_dir().join(format!("casklet-m4-swap-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        let storage_root = root.join("casks");
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(storage_root.clone()),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Swap".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"same bytes").unwrap();
        let managed = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        fs::remove_file(&managed.path).unwrap();
        fs::write(&managed.path, b"same bytes").unwrap();
        let executor = CaskService {
            repository: repository.clone(),
            storage: NoTrash(LocalStorage::new(storage_root)),
        };
        let result = executor
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id,
                    plan_hash: plan.plan_hash,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.items[0].state, "PRESERVED");
        assert_eq!(
            result.items[0].result_code.as_deref(),
            Some("REVALIDATION_FAILED")
        );
        assert!(std::path::Path::new(&managed.path).exists());
        assert_eq!(
            repository.get(&cask.id).unwrap().unwrap().state,
            CaskState::PartiallyDestroyed
        );
        drop(executor);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "integration fixture tests a junction swap between approval and execution"
    )]
    fn execution_preserves_junction_swap_after_approval() {
        struct NoTrash(LocalStorage);
        impl casklet_application::removal::RemovalInspection for NoTrash {
            fn verify_managed(
                &self,
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
            ) -> Option<String> {
                casklet_application::removal::RemovalInspection::verify_managed(
                    &self.0, root, candidate,
                )
            }
            fn trash_verified(
                &self,
                _root: &str,
                _candidate: &casklet_application::removal::RemovalCandidate,
                _identity: &str,
                _run_id: &str,
                _item_id: &str,
            ) -> Result<(), String> {
                panic!("junction must never reach trash")
            }
        }
        let root =
            std::env::temp_dir().join(format!("casklet-m4-junction-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        let storage_root = root.join("casks");
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(storage_root.clone()),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Junction".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"managed bytes").unwrap();
        let managed = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        let external = root.join("external");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("keep.txt"), b"keep").unwrap();
        fs::remove_file(&managed.path).unwrap();
        let junction = std::path::Path::new(&managed.path);
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(junction)
            .arg(&external)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let executor = CaskService {
            repository,
            storage: NoTrash(LocalStorage::new(storage_root)),
        };
        let result = executor
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id,
                    plan_hash: plan.plan_hash,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.items[0].state, "PRESERVED");
        assert_eq!(fs::read(external.join("keep.txt")).unwrap(), b"keep");
        fs::remove_dir(junction).unwrap();
        drop(executor);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "fault fixture verifies uncertain external outcome and retained lock"
    )]
    fn uncertain_trash_outcome_keeps_recovery_lock() {
        struct MoveThenFail {
            storage: LocalStorage,
            stash: PathBuf,
        }
        impl casklet_application::removal::RemovalInspection for MoveThenFail {
            fn verify_managed(
                &self,
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
            ) -> Option<String> {
                casklet_application::removal::RemovalInspection::verify_managed(
                    &self.storage,
                    root,
                    candidate,
                )
            }
            fn trash_verified(
                &self,
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
                identity: &str,
                _run_id: &str,
                _item_id: &str,
            ) -> Result<(), String> {
                if self.verify_managed(root, candidate).as_deref() != Some(identity) {
                    return Err("changed".into());
                }
                fs::rename(&candidate.path, &self.stash).map_err(|e| e.to_string())?;
                Err("simulated uncertain external result".into())
            }
        }
        let root =
            std::env::temp_dir().join(format!("casklet-m4-uncertain-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        let storage_root = root.join("casks");
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(storage_root.clone()),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Uncertain".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"keep source").unwrap();
        let managed = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![],
                    category_key: None,
                },
                "test",
            )
            .unwrap();
        service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id.clone(),
                    plan_hash: plan.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        let stash = root.join("uncertain-stash.txt");
        let executor = CaskService {
            repository: repository.clone(),
            storage: MoveThenFail {
                storage: LocalStorage::new(storage_root),
                stash: stash.clone(),
            },
        };
        let result = executor
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: plan.id,
                    plan_hash: plan.plan_hash,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.state, "PARTIAL");
        assert_eq!(result.items[0].state, "UNKNOWN");
        assert_eq!(
            repository.get(&cask.id).unwrap().unwrap().state,
            CaskState::RecoveryRequired
        );
        let connection = repository.connection().unwrap();
        let locks: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&cask.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(locks, 1);
        assert!(stash.exists());
        assert!(!std::path::Path::new(&managed.path).exists());
        assert!(source.exists());
        drop(connection);
        let closed = executor
            .close_uncertain_removal(
                &CloseRemovalRun {
                    version: CONTRACT_VERSION,
                    run_id: result.run_id.unwrap(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(closed.items[0].state, "PRESERVED");
        assert_eq!(
            repository.get(&cask.id).unwrap().unwrap().state,
            CaskState::PartiallyDestroyed
        );
        let connection = repository.connection().unwrap();
        let remaining_locks: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&cask.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining_locks, 0);
        assert!(stash.exists());
        drop(connection);
        drop(executor);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn upgrades_milestone_one_database_without_losing_casks() {
        let root =
            std::env::temp_dir().join(format!("casklet-m1-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = root.join("state.db");
        let connection = Connection::open(&db).unwrap();
        connection
            .execute_batch(include_str!("../../../migrations/0001_casks.sql"))
            .unwrap();
        connection.execute_batch("PRAGMA user_version=1").unwrap();
        connection.execute("INSERT INTO casks(id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state) VALUES ('existing','Existing','existing','C:/existing',1,1,0,'ACTIVE')", []).unwrap();
        drop(connection);
        let reopened = SqliteCasks::open(db).unwrap();
        assert_eq!(reopened.list().unwrap()[0].name, "Existing");
        assert!(reopened.category_exists("existing", "other").unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn upgrades_milestone_two_pending_intent_without_claiming_success() {
        let root =
            std::env::temp_dir().join(format!("casklet-m3-upgrade-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = root.join("state.db");
        let connection = Connection::open(&db).unwrap();
        connection
            .execute_batch(include_str!("../../../migrations/0001_casks.sql"))
            .unwrap();
        connection
            .execute_batch(include_str!("../../../migrations/0002_resources.sql"))
            .unwrap();
        connection.execute("INSERT INTO casks(id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state) VALUES ('legacy-cask','Legacy','legacy','C:/legacy',1,1,0,'ACTIVE')",[]).unwrap();
        connection.execute("INSERT INTO intake_operations(id,cask_id,resource_id,mode,source_path,staging_path,final_path,state,created_at_ms,updated_at_ms) VALUES ('legacy-op','legacy-cask','legacy-resource','COPY','C:/source','C:/stage','C:/final','PLANNED',1,1)",[]).unwrap();
        connection.execute_batch("PRAGMA user_version=2").unwrap();
        drop(connection);
        let service = CaskService {
            repository: SqliteCasks::open(db).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let operations = service.recovery_status("test").unwrap().operations;
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].state, "RECOVERY_REQUIRED");
        assert_eq!(
            service
                .recover_operation(
                    &RecoverOperation {
                        version: CONTRACT_VERSION,
                        id: "legacy-op".into()
                    },
                    "test"
                )
                .unwrap_err()
                .code,
            "RECOVERY_REQUIRED"
        );
        assert_eq!(
            service
                .preserve_operation(
                    &PreserveOperation {
                        version: CONTRACT_VERSION,
                        id: "legacy-op".into()
                    },
                    "test"
                )
                .unwrap()
                .state,
            "CLOSED_PRESERVED"
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reference_copy_move_and_accounting() {
        let root = std::env::temp_dir().join(format!("casklet-m2-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let reference = root.join("reference.txt");
        fs::write(&reference, b"reference").unwrap();
        let moved = root.join("moved.txt");
        fs::write(&moved, b"moved").unwrap();
        let folder = root.join("folder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("copy.txt"), b"copy").unwrap();
        let add = |source: &std::path::Path, mode| {
            service
                .add_resource(
                    &AddResource {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id.clone(),
                        source_path: source.to_string_lossy().into_owned(),
                        mode,
                        category_key: "other".into(),
                    },
                    "test",
                    &|_, _| {},
                )
                .unwrap()
        };
        let referenced = add(&reference, AddMode::Reference);
        assert_eq!(referenced.management, "REFERENCED");
        assert_eq!(referenced.containment, "EXTERNAL");
        assert_eq!(
            std::path::Path::new(&referenced.path),
            fs::canonicalize(&reference).unwrap()
        );
        assert!(reference.exists());
        let category = service
            .create_category(
                &CreateCategory {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    name: "Notes".into(),
                },
                "test",
            )
            .unwrap();
        let categorized = service
            .categorize_resource(
                &CategorizeResource {
                    version: CONTRACT_VERSION,
                    resource_id: referenced.id,
                    category_key: category.key.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(categorized.category_key, category.key);
        let copied = add(&folder, AddMode::Copy);
        assert_eq!(copied.management, "MANAGED");
        assert!(folder.join("copy.txt").exists());
        assert!(std::path::Path::new(&copied.path).join("copy.txt").exists());
        let moved_item = add(&moved, AddMode::Move);
        assert!(!moved.exists());
        assert_eq!(fs::read(&moved_item.path).unwrap(), b"moved");
        let listed = service
            .list_resources(
                &ListResources {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(listed.len(), 3);
        let sizes = service
            .storage_summary(
                &GetStorageSummary {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                },
                "test",
            )
            .unwrap();
        assert_eq!(sizes.referenced_logical_bytes, 9);
        assert_eq!(sizes.managed_logical_bytes, 9);
        assert_eq!(sizes.physical_bytes, None);
        assert_eq!(sizes.reclaimable_min_bytes, 0);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_intake_preserves_source_on_invalid_input() {
        let root =
            std::env::temp_dir().join(format!("casklet-m2-invalid-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let missing = root.join("missing.txt");
        let failure = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    source_path: missing.to_string_lossy().into_owned(),
                    mode: AddMode::Move,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap_err();
        assert_eq!(failure.code, "SOURCE");
        assert_eq!(service.repository.list().unwrap().len(), 1);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn collision_keeps_source_and_existing_destination() {
        let root =
            std::env::temp_dir().join(format!("casklet-m2-collision-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.txt");
        let destination = root.join("existing.txt");
        fs::write(&source, b"source").unwrap();
        fs::write(&destination, b"existing").unwrap();
        let storage = LocalStorage::new(root.clone());
        let observed = storage
            .inspect(&source.to_string_lossy(), &|_, _| {})
            .unwrap();
        let paths = IntakePaths {
            staging: root.join("staging.txt").to_string_lossy().into_owned(),
            final_path: destination.to_string_lossy().into_owned(),
        };
        assert!(
            storage
                .materialize(&observed, &paths, IntakeMode::Move, &|_, _| {}, &|| false)
                .is_err()
        );
        assert_eq!(fs::read(source).unwrap(), b"source");
        assert_eq!(fs::read(destination).unwrap(), b"existing");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_containing_destination_is_rejected() {
        let root =
            std::env::temp_dir().join(format!("casklet-m2-recursion-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("data.txt"), b"data").unwrap();
        let storage = LocalStorage::new(root.clone());
        let observed = storage
            .inspect(&root.to_string_lossy(), &|_, _| {})
            .unwrap();
        let paths = IntakePaths {
            staging: root.join("staging/copy").to_string_lossy().into_owned(),
            final_path: root.join("managed/copy").to_string_lossy().into_owned(),
        };
        assert!(
            storage
                .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {}, &|| false)
                .is_err()
        );
        assert_eq!(fs::read(root.join("data.txt")).unwrap(), b"data");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_reference_is_not_reported_present() {
        let root =
            std::env::temp_dir().join(format!("casklet-m2-missing-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("reference.txt");
        fs::write(&source, b"reference").unwrap();
        let added = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Reference,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        fs::remove_file(source).unwrap();
        let fetched = service
            .get_resource(
                &GetResource {
                    version: CONTRACT_VERSION,
                    id: added.id,
                },
                "test",
            )
            .unwrap();
        assert_eq!(fetched.availability, "MISSING");
        assert_eq!(fetched.management, "REFERENCED");
        assert_eq!(
            service
                .list_resources(
                    &ListResources {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id.clone()
                    },
                    "test"
                )
                .unwrap()[0]
                .availability,
            "MISSING"
        );
        let summary = service
            .storage_summary(
                &GetStorageSummary {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(summary.referenced_logical_bytes, 0);
        assert_eq!(summary.unknown_entries, 1);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_copy_rejects_link_in_directory() {
        let root = std::env::temp_dir().join(format!("casklet-m2-link-{}", uuid::Uuid::new_v4()));
        let folder = root.join("folder");
        fs::create_dir_all(&folder).unwrap();
        let external = root.join("external.txt");
        fs::write(&external, b"preserve").unwrap();
        let link = folder.join("link.txt");
        #[cfg(windows)]
        let linked = std::os::windows::fs::symlink_file(&external, &link).is_ok();
        #[cfg(unix)]
        let linked = std::os::unix::fs::symlink(&external, &link).is_ok();
        if linked {
            let storage = LocalStorage::new(root.clone());
            let observed = storage
                .inspect(&folder.to_string_lossy(), &|_, _| {})
                .unwrap();
            assert!(observed.unknown_entries > 0);
            let paths = IntakePaths {
                staging: root.join("staging").to_string_lossy().into_owned(),
                final_path: root.join("managed").to_string_lossy().into_owned(),
            };
            assert!(
                storage
                    .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {}, &|| false)
                    .is_err()
            );
            assert_eq!(fs::read(&external).unwrap(), b"preserve");
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn managed_copy_rejects_windows_junction() {
        let root =
            std::env::temp_dir().join(format!("casklet-m2-junction-{}", uuid::Uuid::new_v4()));
        let folder = root.join("folder");
        let external = root.join("external");
        fs::create_dir_all(&folder).unwrap();
        fs::create_dir(&external).unwrap();
        fs::write(external.join("preserve.txt"), b"preserve").unwrap();
        let junction = folder.join("junction");
        let output = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&external)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let storage = LocalStorage::new(root.clone());
        let observed = storage
            .inspect(&folder.to_string_lossy(), &|_, _| {})
            .unwrap();
        assert!(observed.unknown_entries > 0);
        let paths = IntakePaths {
            staging: root.join("staging").to_string_lossy().into_owned(),
            final_path: root.join("managed").to_string_lossy().into_owned(),
        };
        assert!(
            storage
                .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {}, &|| false)
                .is_err()
        );
        assert_eq!(
            fs::read(external.join("preserve.txt")).unwrap(),
            b"preserve"
        );
        fs::remove_dir(&junction).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_copy_keeps_source_and_intake_intent() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let root =
            std::env::temp_dir().join(format!("casklet-m2-interrupt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("large.bin");
        fs::write(&source, vec![7_u8; 2 * 1024 * 1024]).unwrap();
        let notifications = AtomicUsize::new(0);
        let interrupted = service.add_resource(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: cask.id.clone(),
                source_path: source.to_string_lossy().into_owned(),
                mode: AddMode::Copy,
                category_key: "other".into(),
            },
            "test",
            &|_, _| {
                assert_ne!(
                    notifications.fetch_add(1, Ordering::SeqCst),
                    1,
                    "simulated interruption"
                );
            },
        );
        assert_eq!(interrupted.unwrap_err().code, "INTAKE_RECOVERY_REQUIRED");
        assert_eq!(fs::metadata(&source).unwrap().len(), 2 * 1024 * 1024);
        let (state, staged): (String, String) = service
            .repository
            .connection()
            .unwrap()
            .query_row(
                "SELECT state,staging_path FROM intake_operations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(state, "RECOVERY_REQUIRED");
        assert!(std::path::Path::new(&staged).exists());
        let op_id: String = service
            .repository
            .connection()
            .unwrap()
            .query_row("SELECT id FROM intake_operations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            service
                .recover_operation(
                    &RecoverOperation {
                        version: CONTRACT_VERSION,
                        id: op_id
                    },
                    "test"
                )
                .unwrap_err()
                .code,
            "RECOVERY_REQUIRED"
        );
        assert!(
            service
                .list_resources(
                    &ListResources {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id
                    },
                    "test"
                )
                .unwrap()
                .is_empty()
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn concurrent_intake_is_locked_and_manifest_is_projected() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let root = std::env::temp_dir().join(format!("casklet-m3-lock-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let first = root.join("first.bin");
        let second = root.join("second.bin");
        fs::write(&first, vec![1_u8; 2 * 1024 * 1024]).unwrap();
        fs::write(&second, b"second").unwrap();
        let attempted = AtomicBool::new(false);
        let notifications = AtomicUsize::new(0);
        let result = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: first.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, total| {
                    if total > 0
                        && notifications.fetch_add(1, Ordering::SeqCst) == 1
                        && !attempted.swap(true, Ordering::SeqCst)
                    {
                        let conflict = service
                            .add_resource(
                                &AddResource {
                                    version: CONTRACT_VERSION,
                                    cask_id: cask.id.clone(),
                                    source_path: second.to_string_lossy().into_owned(),
                                    mode: AddMode::Move,
                                    category_key: "other".into(),
                                },
                                "test",
                                &|_, _| {},
                            )
                            .unwrap_err();
                        assert_eq!(conflict.code, "PERSISTENCE");
                    }
                },
            )
            .unwrap();
        assert!(attempted.load(Ordering::SeqCst));
        assert!(second.exists());
        let operations = service
            .list_operations(
                &ListOperations {
                    version: CONTRACT_VERSION,
                    cask_id: Some(cask.id.clone()),
                },
                "test",
            )
            .unwrap();
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0].state, "SUCCEEDED");
        let lock_count: i64 = service
            .repository
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM operation_locks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lock_count, 0);
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(PathBuf::from(&cask.managed_storage_root).join(".casklet/manifest.json"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["resources"][0]["id"], result.id);
        assert!(
            service
                .recovery_status("test")
                .unwrap()
                .pending_manifest_casks
                .is_empty()
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_move_preserves_staging_and_can_close_without_deletion() {
        let root = std::env::temp_dir().join(format!("casklet-m3-move-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"preserve").unwrap();
        let observed = service
            .storage
            .inspect(&source.to_string_lossy(), &|_, _| {})
            .unwrap();
        let mut resource = Resource::new(
            cask.id.clone(),
            observed.name.clone(),
            observed.source_path.clone(),
            observed.kind,
            IntakeMode::Move,
            "other".into(),
            observed.size_bytes,
            0,
            1,
        )
        .unwrap();
        let paths = service.storage.allocate(
            &service.repository.get(&cask.id).unwrap().unwrap(),
            &resource.id,
            &resource.name,
        );
        resource.path.clone_from(&paths.final_path);
        let op = uuid::Uuid::new_v4().to_string();
        service
            .repository
            .begin_intake(
                &op,
                &resource,
                IntakeMode::Move,
                &observed.source_path,
                &paths.staging,
                &paths.final_path,
            )
            .unwrap();
        fs::create_dir_all(PathBuf::from(&paths.staging).parent().unwrap()).unwrap();
        fs::rename(&source, &paths.staging).unwrap();
        fs::write(&source, b"ambiguous new source").unwrap();
        service
            .repository
            .connection()
            .unwrap()
            .execute("UPDATE operation_locks SET owner_instance_id='999999'", [])
            .unwrap();
        service.repository.detect_abandoned_operations().unwrap();
        assert_eq!(
            service.recovery_status("test").unwrap().operations[0].state,
            "RECOVERY_REQUIRED"
        );
        let error = service
            .recover_operation(
                &RecoverOperation {
                    version: CONTRACT_VERSION,
                    id: op.clone(),
                },
                "test",
            )
            .unwrap_err();
        assert_eq!(error.code, "RECOVERY_REQUIRED");
        let preserved = service
            .preserve_operation(
                &PreserveOperation {
                    version: CONTRACT_VERSION,
                    id: op,
                },
                "test",
            )
            .unwrap();
        assert_eq!(preserved.state, "CLOSED_PRESERVED");
        assert!(PathBuf::from(&paths.staging).exists());
        assert_eq!(fs::read(&source).unwrap(), b"ambiguous new source");
        let lock_count: i64 = service
            .repository
            .connection()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM operation_locks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(lock_count, 0);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "staged recovery test shows each durable boundary"
    )]
    fn complete_staging_recovers_idempotently_after_restart() {
        let root = std::env::temp_dir().join(format!("casklet-m3-stage-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"complete stage").unwrap();
        let observed = service
            .storage
            .inspect(&source.to_string_lossy(), &|_, _| {})
            .unwrap();
        let mut resource = Resource::new(
            cask.id.clone(),
            observed.name.clone(),
            observed.source_path.clone(),
            observed.kind,
            IntakeMode::Move,
            "other".into(),
            observed.size_bytes,
            0,
            1,
        )
        .unwrap();
        let paths = service.storage.allocate(
            &service.repository.get(&cask.id).unwrap().unwrap(),
            &resource.id,
            &resource.name,
        );
        resource.path.clone_from(&paths.final_path);
        let op = uuid::Uuid::new_v4().to_string();
        service
            .repository
            .begin_intake(
                &op,
                &resource,
                IntakeMode::Move,
                &observed.source_path,
                &paths.staging,
                &paths.final_path,
            )
            .unwrap();
        fs::create_dir_all(PathBuf::from(&paths.staging).parent().unwrap()).unwrap();
        fs::rename(&source, &paths.staging).unwrap();
        service
            .repository
            .connection()
            .unwrap()
            .execute("UPDATE operation_locks SET owner_instance_id='999999'", [])
            .unwrap();
        drop(service);
        let reopened = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        reopened.repository.detect_abandoned_operations().unwrap();
        let recovered = reopened
            .recover_operation(
                &RecoverOperation {
                    version: CONTRACT_VERSION,
                    id: op.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(recovered.state, "SUCCEEDED");
        assert_eq!(fs::read(&paths.final_path).unwrap(), b"complete stage");
        assert!(!source.exists());
        assert!(!PathBuf::from(&paths.staging).exists());
        assert_eq!(
            reopened
                .recover_operation(
                    &RecoverOperation {
                        version: CONTRACT_VERSION,
                        id: op
                    },
                    "test"
                )
                .unwrap()
                .state,
            "SUCCEEDED"
        );
        assert_eq!(
            reopened
                .list_resources(
                    &ListResources {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id
                    },
                    "test"
                )
                .unwrap()
                .len(),
            1
        );
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unregistered_final_can_be_verified_and_registered() {
        let root = std::env::temp_dir().join(format!("casklet-m3-final-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"complete final").unwrap();
        let observed = service
            .storage
            .inspect(&source.to_string_lossy(), &|_, _| {})
            .unwrap();
        let mut resource = Resource::new(
            cask.id.clone(),
            observed.name.clone(),
            observed.source_path.clone(),
            observed.kind,
            IntakeMode::Copy,
            "other".into(),
            observed.size_bytes,
            0,
            1,
        )
        .unwrap();
        let paths = service.storage.allocate(
            &service.repository.get(&cask.id).unwrap().unwrap(),
            &resource.id,
            &resource.name,
        );
        resource.path.clone_from(&paths.final_path);
        let op = uuid::Uuid::new_v4().to_string();
        service
            .repository
            .begin_intake(
                &op,
                &resource,
                IntakeMode::Copy,
                &observed.source_path,
                &paths.staging,
                &paths.final_path,
            )
            .unwrap();
        service
            .storage
            .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {}, &|| false)
            .unwrap();
        service
            .repository
            .connection()
            .unwrap()
            .execute("UPDATE operation_locks SET owner_instance_id='999999'", [])
            .unwrap();
        service.repository.detect_abandoned_operations().unwrap();
        assert_eq!(
            service
                .recover_operation(
                    &RecoverOperation {
                        version: CONTRACT_VERSION,
                        id: op
                    },
                    "test"
                )
                .unwrap()
                .state,
            "SUCCEEDED"
        );
        assert_eq!(fs::read(&paths.final_path).unwrap(), b"complete final");
        assert!(source.exists());
        assert_eq!(
            service
                .list_resources(
                    &ListResources {
                        version: CONTRACT_VERSION,
                        cask_id: cask.id
                    },
                    "test"
                )
                .unwrap()
                .len(),
            1
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "restart recovery test keeps each durable boundary visible"
    )]
    fn registered_intake_can_be_verified_after_restart() {
        let root =
            std::env::temp_dir().join(format!("casklet-m3-registered-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"registered").unwrap();
        let observed = service
            .storage
            .inspect(&source.to_string_lossy(), &|_, _| {})
            .unwrap();
        let mut resource = Resource::new(
            cask.id.clone(),
            observed.name.clone(),
            observed.source_path.clone(),
            observed.kind,
            IntakeMode::Copy,
            "other".into(),
            observed.size_bytes,
            0,
            1,
        )
        .unwrap();
        let paths = service.storage.allocate(
            &service.repository.get(&cask.id).unwrap().unwrap(),
            &resource.id,
            &resource.name,
        );
        resource.path.clone_from(&paths.final_path);
        let op = uuid::Uuid::new_v4().to_string();
        service
            .repository
            .begin_intake(
                &op,
                &resource,
                IntakeMode::Copy,
                &observed.source_path,
                &paths.staging,
                &paths.final_path,
            )
            .unwrap();
        service
            .storage
            .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {}, &|| false)
            .unwrap();
        service.repository.materialized_intake(&op).unwrap();
        service
            .repository
            .commit_resource(&resource, Some(&op))
            .unwrap();
        service
            .repository
            .connection()
            .unwrap()
            .execute("UPDATE operation_locks SET owner_instance_id='999999'", [])
            .unwrap();
        drop(service);
        let reopened = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        reopened.repository.detect_abandoned_operations().unwrap();
        let recovered = reopened
            .recover_operation(
                &RecoverOperation {
                    version: CONTRACT_VERSION,
                    id: op.clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(recovered.state, "SUCCEEDED");
        assert_eq!(
            reopened
                .recover_operation(
                    &RecoverOperation {
                        version: CONTRACT_VERSION,
                        id: op
                    },
                    "test"
                )
                .unwrap()
                .state,
            "SUCCEEDED"
        );
        assert!(
            reopened
                .recovery_status("test")
                .unwrap()
                .operations
                .is_empty()
        );
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manifest_projection_failure_remains_pending_and_retries() {
        let root =
            std::env::temp_dir().join(format!("casklet-m3-projection-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let metadata = PathBuf::from(&cask.managed_storage_root).join(".casklet");
        fs::remove_dir_all(&metadata).unwrap();
        fs::write(&metadata, b"blocked").unwrap();
        let source = root.join("reference.txt");
        fs::write(&source, b"reference").unwrap();
        let added = service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Reference,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        assert_eq!(
            service
                .recovery_status("test")
                .unwrap()
                .pending_manifest_casks,
            vec![cask.id.clone()]
        );
        fs::remove_file(&metadata).unwrap();
        assert_eq!(service.retry_manifest_projections("test").unwrap(), 1);
        assert!(
            service
                .recovery_status("test")
                .unwrap()
                .pending_manifest_casks
                .is_empty()
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(metadata.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["resources"][0]["id"], added.id);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_manifest_swap_restores_valid_previous_projection() {
        let root =
            std::env::temp_dir().join(format!("casklet-m3-manifest-swap-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let metadata = PathBuf::from(&cask.managed_storage_root).join(".casklet");
        fs::rename(
            metadata.join("manifest.json"),
            metadata.join("manifest.valid.prev"),
        )
        .unwrap();
        fs::write(metadata.join("manifest.invalid.prev"), b"not JSON").unwrap();
        service
            .repository
            .connection()
            .unwrap()
            .execute(
                "UPDATE projection_jobs SET state='PENDING' WHERE cask_id=?1",
                [&cask.id],
            )
            .unwrap();
        assert_eq!(service.retry_manifest_projections("test").unwrap(), 1);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(metadata.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["cask"]["id"], cask.id);
        assert!(!metadata.join("manifest.valid.prev").exists());
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn hard_exit_boundary_helper() {
        let Ok(phase) = std::env::var("CASKLET_TEST_EXIT_PHASE") else {
            return;
        };
        let root = PathBuf::from(std::env::var("CASKLET_TEST_EXIT_ROOT").unwrap());
        let cask_id = std::env::var("CASKLET_TEST_EXIT_CASK").unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let source = root.join("source.bin");
        let observed = service
            .storage
            .inspect(&source.to_string_lossy(), &|_, _| {})
            .unwrap();
        let mut resource = Resource::new(
            cask_id.clone(),
            observed.name.clone(),
            observed.source_path.clone(),
            observed.kind,
            IntakeMode::Copy,
            "other".into(),
            observed.size_bytes,
            0,
            1,
        )
        .unwrap();
        let paths = service.storage.allocate(
            &service.repository.get(&cask_id).unwrap().unwrap(),
            &resource.id,
            &resource.name,
        );
        resource.path.clone_from(&paths.final_path);
        let op = uuid::Uuid::new_v4().to_string();
        service
            .repository
            .begin_intake(
                &op,
                &resource,
                IntakeMode::Copy,
                &observed.source_path,
                &paths.staging,
                &paths.final_path,
            )
            .unwrap();
        if phase == "STAGE" {
            fs::create_dir_all(PathBuf::from(&paths.staging).parent().unwrap()).unwrap();
            fs::copy(&source, &paths.staging).unwrap();
        }
        if matches!(phase.as_str(), "FINAL" | "REGISTER" | "COMPLETE") {
            service
                .storage
                .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {}, &|| false)
                .unwrap();
        }
        if matches!(phase.as_str(), "REGISTER" | "COMPLETE") {
            service.repository.materialized_intake(&op).unwrap();
            service
                .repository
                .commit_resource(&resource, Some(&op))
                .unwrap();
        }
        if phase == "COMPLETE" {
            service.repository.complete_intake(&op).unwrap();
        }
        std::process::exit(73);
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "child-process fault matrix records every intake boundary"
    )]
    fn forced_exit_at_each_durable_boundary_is_recoverable() {
        for phase in ["INTENT", "STAGE", "FINAL", "REGISTER", "COMPLETE"] {
            let root = std::env::temp_dir()
                .join(format!("casklet-m3-exit-{phase}-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let service = CaskService {
                repository: SqliteCasks::open(root.join("state.db")).unwrap(),
                storage: LocalStorage::new(root.join("casks")),
            };
            let cask = service
                .create(
                    &CreateCask {
                        version: CONTRACT_VERSION,
                        name: "Work".into(),
                    },
                    "test",
                )
                .unwrap();
            fs::write(root.join("source.bin"), b"durable bytes").unwrap();
            drop(service);
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "tests::hard_exit_boundary_helper", "--nocapture"])
                .env("CASKLET_TEST_EXIT_PHASE", phase)
                .env("CASKLET_TEST_EXIT_ROOT", &root)
                .env("CASKLET_TEST_EXIT_CASK", &cask.id)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(73),
                "phase {phase}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let service = CaskService {
                repository: SqliteCasks::open(root.join("state.db")).unwrap(),
                storage: LocalStorage::new(root.join("casks")),
            };
            service.repository.detect_abandoned_operations().unwrap();
            let op = service
                .list_operations(
                    &ListOperations {
                        version: CONTRACT_VERSION,
                        cask_id: Some(cask.id.clone()),
                    },
                    "test",
                )
                .unwrap()
                .remove(0);
            if phase == "INTENT" {
                assert_eq!(op.state, "RECOVERY_REQUIRED");
                assert_eq!(
                    service
                        .recover_operation(
                            &RecoverOperation {
                                version: CONTRACT_VERSION,
                                id: op.id.clone()
                            },
                            "test"
                        )
                        .unwrap_err()
                        .code,
                    "RECOVERY_REQUIRED"
                );
                assert_eq!(
                    service
                        .preserve_operation(
                            &PreserveOperation {
                                version: CONTRACT_VERSION,
                                id: op.id
                            },
                            "test"
                        )
                        .unwrap()
                        .state,
                    "CLOSED_PRESERVED"
                );
            } else {
                let recovered = service
                    .recover_operation(
                        &RecoverOperation {
                            version: CONTRACT_VERSION,
                            id: op.id,
                        },
                        "test",
                    )
                    .unwrap();
                assert_eq!(recovered.state, "SUCCEEDED");
                assert_eq!(
                    service
                        .list_resources(
                            &ListResources {
                                version: CONTRACT_VERSION,
                                cask_id: cask.id.clone()
                            },
                            "test"
                        )
                        .unwrap()
                        .len(),
                    1
                );
                assert!(root.join("source.bin").exists());
            }
            let _ = service.retry_manifest_projections("test").unwrap();
            assert!(
                service
                    .recovery_status("test")
                    .unwrap()
                    .pending_manifest_casks
                    .is_empty()
            );
            drop(service);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn cancellation_stops_copy_at_boundary_and_keeps_source() {
        use std::sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        };
        let root = std::env::temp_dir().join(format!("casklet-m3-cancel-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("large.bin");
        fs::write(&source, vec![3_u8; 3 * 1024 * 1024]).unwrap();
        let operation_id = Mutex::new(String::new());
        let requested = AtomicBool::new(false);
        let result = service.add_resource_with_events(
            &AddResource {
                version: CONTRACT_VERSION,
                cask_id: cask.id.clone(),
                source_path: source.to_string_lossy().into_owned(),
                mode: AddMode::Copy,
                category_key: "other".into(),
            },
            "test",
            &|done, total| {
                if done < total
                    && !operation_id.lock().unwrap().is_empty()
                    && !requested.swap(true, Ordering::SeqCst)
                {
                    let id = operation_id.lock().unwrap().clone();
                    service
                        .cancel_operation(
                            &CancelOperation {
                                version: CONTRACT_VERSION,
                                id,
                            },
                            "test",
                        )
                        .unwrap();
                }
            },
            &|id| {
                *operation_id.lock().unwrap() = id.into();
            },
        );
        assert_eq!(result.unwrap_err().code, "INTAKE_RECOVERY_REQUIRED");
        assert!(requested.load(Ordering::SeqCst));
        assert_eq!(fs::metadata(&source).unwrap().len(), 3 * 1024 * 1024);
        let inspection = service
            .inspect_recovery(
                &casklet_contracts::GetOperation {
                    version: CONTRACT_VERSION,
                    id: operation_id.lock().unwrap().clone(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(inspection.staging_status, "PRESENT_FILE");
        assert_eq!(inspection.final_status, "MISSING");
        assert_eq!(inspection.operation.state, "RECOVERY_REQUIRED");
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn diagnostics_bundle_contains_codes_without_private_paths() {
        let root =
            std::env::temp_dir().join(format!("casklet-m3-diagnostics-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Work".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("private-secret-name.txt");
        fs::write(&source, b"private").unwrap();
        service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let output = root.join("diagnostics.json");
        service
            .create_diagnostics_bundle(
                &CreateDiagnosticsBundle {
                    version: CONTRACT_VERSION,
                    output_path: output.to_string_lossy().into_owned(),
                },
                "test",
            )
            .unwrap();
        let bundle = fs::read_to_string(&output).unwrap();
        assert!(bundle.contains("INTAKE_STARTED"));
        assert!(bundle.contains("INTAKE_SUCCEEDED"));
        assert!(!bundle.contains("private-secret-name"));
        assert!(
            service
                .create_diagnostics_bundle(
                    &CreateDiagnosticsBundle {
                        version: CONTRACT_VERSION,
                        output_path: output.to_string_lossy().into_owned()
                    },
                    "test"
                )
                .is_err()
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }
}
