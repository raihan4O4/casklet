//! SQLite and managed-storage adapters.
use casklet_application::{CaskRepository, CaskService, ManagedStorage};
use casklet_domain::{Cask, CaskState};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, time::Duration};

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
            1 => {}
            _ => return Err(format!("unsupported database schema version: {version}")),
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
    use casklet_contracts::{CONTRACT_VERSION, CreateCask, GetCask, ListCasks};
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
        assert_eq!(schema_version, 1);
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
}
