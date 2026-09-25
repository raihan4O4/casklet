//! SQLite and managed-storage adapters.
use casklet_application::{CaskRepository, CaskService, ManagedStorage};
use casklet_domain::{Cask, CaskState};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::Duration};
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
            1 | 2 => {}
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
        self.connection()?.execute("INSERT INTO casks (id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![cask.id,cask.name,cask.storage_key,cask.managed_storage_root,cask.created_at_ms,cask.updated_at_ms,cask.revision,cask.state.code()]).map_err(|e| e.to_string())?;
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
    Ok(CaskService {
        repository: SqliteCasks::open(root.join("db/casklet.db"))?,
        storage: LocalStorage::new(root.join("casks")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::resource::{IntakePaths, ResourceRepository, ResourceStorage};
    use casklet_contracts::{
        AddMode, AddResource, CONTRACT_VERSION, CategorizeResource, CreateCask, CreateCategory,
        GetCask, GetResource, GetStorageSummary, ListCasks, ListResources,
    };
    use casklet_domain::resource::IntakeMode;
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
        assert_eq!(schema_version, 2);
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
                .materialize(&observed, &paths, IntakeMode::Move, &|_, _| {})
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
                .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {})
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
                    .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {})
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
                .materialize(&observed, &paths, IntakeMode::Copy, &|_, _| {})
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
        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = service.add_resource(
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
        }));
        assert!(interrupted.is_err());
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
        assert_eq!(state, "PLANNED");
        assert!(std::path::Path::new(&staged).exists());
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
}
