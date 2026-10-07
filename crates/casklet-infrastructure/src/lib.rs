//! SQLite and managed-storage adapters.
use casklet_application::backup::DatabaseBackupRepository;
use casklet_application::projection::ManifestProjection;
use casklet_application::{CaskRepository, CaskService, ManagedStorage};
use casklet_domain::{Cask, CaskState};
use rusqlite::{Connection, params};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
mod diagnostics;
mod installer;
mod installer_source;
pub mod inventory;
mod managed_msi;
mod msi_uninstaller;
mod operations;
#[cfg(all(test, windows))]
mod ownership_tests;
mod portable;
pub mod portable_zip;
mod projection;
mod removal;
mod resource_storage;
mod resources;
mod windows_inventory;
mod windows_process;
mod winget;
mod winget_cli;

pub const CRATE_BOUNDARY: &str = "infrastructure";

#[derive(Debug, Clone)]
pub struct SqliteCasks {
    db_path: PathBuf,
}

const MIGRATIONS: &[&str] = &[
    include_str!("../../../migrations/0001_casks.sql"),
    include_str!("../../../migrations/0002_resources.sql"),
    include_str!("../../../migrations/0003_operations.sql"),
    include_str!("../../../migrations/0004_removal_plans.sql"),
    include_str!("../../../migrations/0005_removal_execution.sql"),
    include_str!("../../../migrations/0006_permanent_disposal.sql"),
    include_str!("../../../migrations/0007_portable_installations.sql"),
    include_str!("../../../migrations/0008_windows_inventory.sql"),
    include_str!("../../../migrations/0009_winget_lifecycle.sql"),
    include_str!("../../../migrations/0010_installer_artifacts.sql"),
    include_str!("../../../migrations/0011_cask_metadata.sql"),
    include_str!("../../../migrations/0012_ownership_and_cleanup.sql"),
    include_str!("../../../migrations/0013_cask_removal.sql"),
    include_str!("../../../migrations/0014_managed_msi.sql"),
    include_str!("../../../migrations/0015_installer_provenance.sql"),
    include_str!("../../../migrations/0016_application_lifecycle.sql"),
];

impl SqliteCasks {
    /// # Errors
    /// Returns an I/O or migration error.
    pub fn open(db_path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let this = Self { db_path };
        // Inspect compatibility before changing journal mode or migrating. A
        // newer database must remain untouched when an older build opens it.
        let connection = Connection::open(&this.db_path).map_err(|e| e.to_string())?;
        let stored_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let version = usize::try_from(stored_version)
            .map_err(|_| format!("unsupported database schema version: {stored_version}"))?;
        let integrity: String = connection
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|e| format!("database integrity check failed: {e}"))?;
        if integrity != "ok" {
            return Err(format!("database integrity check failed: {integrity}"));
        }
        if version > MIGRATIONS.len() {
            return Err(format!(
                "unsupported database schema version: {version}; this Casklet build supports up to {}. Install the matching or newer Casklet setup. The database has not been downgraded",
                MIGRATIONS.len()
            ));
        }
        drop(connection);
        let mut connection = this.connection()?;
        if version > 0 && version < MIGRATIONS.len() {
            backup_before_migration(
                &connection,
                &this.db_path,
                i64::try_from(version).map_err(|e| e.to_string())?,
            )?;
        }
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(version) {
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            transaction
                .execute_batch(sql)
                .map_err(|e| format!("migration {} failed: {e}", index + 1))?;
            transaction
                .pragma_update(
                    None,
                    "user_version",
                    i64::try_from(index + 1).map_err(|e| e.to_string())?,
                )
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

fn backup_before_migration(
    connection: &Connection,
    db_path: &Path,
    version: i64,
) -> Result<(), String> {
    create_database_backup(connection, db_path, &format!("pre-v{version}"))?;
    Ok(())
}

fn create_database_backup(
    connection: &Connection,
    db_path: &Path,
    label: &str,
) -> Result<(PathBuf, u64), String> {
    let integrity: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| format!("source integrity check failed: {e}"))?;
    if integrity != "ok" {
        return Err(format!("source integrity check failed: {integrity}"));
    }
    let db_dir = db_path.parent().ok_or("database has no parent directory")?;
    let backup_parent = if db_dir.file_name().is_some_and(|name| name == "db") {
        db_dir.parent().unwrap_or(db_dir)
    } else {
        db_dir
    };
    let backup_dir = backup_parent.join("backups");
    fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
    let backup_path = backup_dir.join(format!("casklet-{label}-{}.db", uuid::Uuid::new_v4()));
    connection
        .backup(rusqlite::MAIN_DB, &backup_path, None)
        .map_err(|e| format!("database backup failed: {e}"))?;
    let verified =
        Connection::open(&backup_path).map_err(|e| format!("backup could not be reopened: {e}"))?;
    verified
        .execute_batch("PRAGMA journal_mode=DELETE")
        .map_err(|e| format!("backup journal could not be finalized: {e}"))?;
    let backup_integrity: String = verified
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| format!("backup integrity check failed: {e}"))?;
    if backup_integrity != "ok" {
        return Err(format!("backup integrity check failed: {backup_integrity}"));
    }
    let size = fs::metadata(&backup_path).map_err(|e| e.to_string())?.len();
    Ok((backup_path, size))
}

impl DatabaseBackupRepository for SqliteCasks {
    fn backup_database(&self) -> Result<(String, u64), String> {
        let (path, size) = create_database_backup(&self.connection()?, &self.db_path, "manual")?;
        Ok((path.to_string_lossy().into_owned(), size))
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
        description: row.get(8)?,
        storage_key: row.get(2)?,
        managed_storage_root: row.get(3)?,
        created_at_ms: row.get(4)?,
        updated_at_ms: row.get(5)?,
        revision: row.get(6)?,
        state,
    })
}
const SELECT: &str = "SELECT id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state,description FROM casks";

fn cask_removal_status(
    connection: &Connection,
    id: &str,
) -> Result<casklet_contracts::CaskRemovalStatus, String> {
    let (eligible,resources,applications,unfinished): (bool,i64,i64,i64) = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM casks WHERE id=?1 AND state IN ('ACTIVE','ARCHIVED','PARTIALLY_DESTROYED','DESTROYED') AND NOT EXISTS(SELECT 1 FROM cask_removals WHERE cask_id=?1)),
        (SELECT COUNT(*) FROM resource_associations WHERE cask_id=?1 AND removed_at_ms IS NULL),
        (SELECT COUNT(*) FROM managed_msi_installations m WHERE cask_id=?1 AND state='OWNED' AND NOT EXISTS(SELECT 1 FROM windows_cask_associations a WHERE a.cask_id=m.cask_id AND a.installation_id=m.installation_id AND a.removed_at_ms IS NULL)) + (SELECT COUNT(*) FROM cask_installations WHERE cask_id=?1 AND removed_at_ms IS NULL) + (SELECT COUNT(*) FROM windows_cask_associations WHERE cask_id=?1 AND removed_at_ms IS NULL) + (SELECT COUNT(*) FROM winget_installations WHERE cask_id=?1 AND state!='REMOVED'),
        (SELECT COUNT(*) FROM operation_locks WHERE lock_key='cask:'||?1||':mutation') + (SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1) + (SELECT COUNT(*) FROM installer_locks WHERE cask_id=?1) + (SELECT COUNT(*) FROM winget_locks WHERE cask_id=?1) + (SELECT COUNT(*) FROM operations WHERE cask_id=?1 AND state IN ('RUNNING','RECOVERY_REQUIRED')) + (SELECT COUNT(*) FROM installer_operations WHERE cask_id=?1 AND state IN ('RUNNING','REVIEW_REQUIRED','RECOVERY_REQUIRED')) + (SELECT COUNT(*) FROM winget_operations WHERE cask_id=?1 AND state IN ('RUNNING','RECOVERY_REQUIRED')) + (SELECT COUNT(*) FROM removal_runs WHERE cask_id=?1 AND state IN ('RUNNING','RECOVERY_REQUIRED')) + (SELECT COUNT(*) FROM msi_cleanup_locks WHERE cask_id=?1)", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).map_err(|e|e.to_string())?;
    Ok(casklet_contracts::CaskRemovalStatus {
        version: casklet_contracts::CONTRACT_VERSION,
        can_remove: eligible && resources == 0 && applications == 0 && unfinished == 0,
        resource_count: u64::try_from(resources).map_err(|e| e.to_string())?,
        application_count: u64::try_from(applications).map_err(|e| e.to_string())?,
        unfinished_operations: u64::try_from(unfinished).map_err(|e| e.to_string())?,
    })
}

impl CaskRepository for SqliteCasks {
    fn record_storage_retirement(
        &self,
        id: &str,
        warning: Option<&str>,
        now_ms: i64,
    ) -> Result<(), String> {
        self.connection()?.execute("UPDATE cask_removals SET storage_warning=?2,storage_retired_at_ms=CASE WHEN ?2 IS NULL THEN ?3 ELSE NULL END WHERE cask_id=?1",params![id,warning,now_ms]).map_err(|e|e.to_string())?;
        Ok(())
    }
    fn removal_status(&self, id: &str) -> Result<casklet_contracts::CaskRemovalStatus, String> {
        cask_removal_status(&self.connection()?, id)
    }
    fn remove_empty(&self, id: &str, expected_revision: i64, now_ms: i64) -> Result<bool, String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        if !cask_removal_status(&transaction, id)?.can_remove {
            return Ok(false);
        }
        let changed = transaction.execute("UPDATE casks SET state='DESTROYED',revision=revision+1,updated_at_ms=?3 WHERE id=?1 AND revision=?2 AND NOT EXISTS(SELECT 1 FROM cask_removals WHERE cask_id=?1)", params![id,expected_revision,now_ms]).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Ok(false);
        }
        transaction.execute("INSERT INTO cask_removals(cask_id,removed_at_ms,expected_revision) VALUES (?1,?2,?3)",params![id,now_ms,expected_revision]).map_err(|e| e.to_string())?;
        transaction.execute("UPDATE removal_plans SET state='INVALIDATED',revision=revision+1 WHERE cask_id=?1 AND state IN ('DRAFT','APPROVED')",[id]).map_err(|e|e.to_string())?;
        transaction
            .execute("DELETE FROM projection_jobs WHERE cask_id=?1", [id])
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(true)
    }

    fn update(&self, cask: &Cask, expected_revision: i64) -> Result<bool, String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction.execute("UPDATE casks SET name=?1,description=?2,state=?3,updated_at_ms=?4,revision=?5 WHERE id=?6 AND revision=?7 AND NOT EXISTS (SELECT 1 FROM cask_removals WHERE cask_id=?6) AND NOT EXISTS (SELECT 1 FROM operation_locks WHERE lock_key=?8) AND NOT EXISTS (SELECT 1 FROM removal_locks WHERE cask_id=?6) AND NOT EXISTS (SELECT 1 FROM installer_locks WHERE cask_id=?6) AND NOT EXISTS (SELECT 1 FROM msi_cleanup_locks WHERE cask_id=?6)", params![cask.name,cask.description,cask.state.code(),cask.updated_at_ms,cask.revision,cask.id,expected_revision,format!("cask:{}:mutation",cask.id)]).map_err(|e|e.to_string())?;
        if changed == 1 {
            transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,?2,'PENDING',?3) ON CONFLICT(cask_id) DO UPDATE SET target_revision=excluded.target_revision,state='PENDING',updated_at_ms=excluded.updated_at_ms", params![cask.id,cask.revision,cask.updated_at_ms]).map_err(|e|e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(changed == 1)
    }
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
            .query_row(&format!("{SELECT} WHERE id=?1 AND NOT EXISTS (SELECT 1 FROM cask_removals WHERE cask_id=casks.id)"), [id], row)
            .optional()
            .map_err(|e| e.to_string())
    }
    fn list(&self) -> Result<Vec<Cask>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(&format!("{SELECT} WHERE NOT EXISTS (SELECT 1 FROM cask_removals WHERE cask_id=casks.id) ORDER BY created_at_ms,id"))
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
impl casklet_application::library::StorageBrowser for LocalStorage {
    fn storage_root(&self) -> String {
        self.root.to_string_lossy().into_owned()
    }
    fn open_directory(&self, path: &str) -> Result<(), String> {
        let directory = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
        let root = self.root.canonicalize().map_err(|e| e.to_string())?;
        if !directory.is_dir() || !directory.starts_with(root) {
            return Err("Storage directory is unavailable or outside the configured root".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let windows = std::env::var_os("WINDIR").ok_or("Windows directory unavailable")?;
            std::process::Command::new(PathBuf::from(windows).join("explorer.exe"))
                .arg(directory)
                .creation_flags(0x0800_0000)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err("Opening storage is supported on Windows only".into())
        }
    }
}
fn hold_retirement_directory(path: &Path) -> Result<Option<fs::File>, String> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Deny directory rename/removal while enumerating its children. User bytes
        // are never removed; a newly created entry simply makes remove_dir fail.
        fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x0200_0000)
            .open(path)
            .map(Some)
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(None)
    }
}
fn prune_empty(path: &Path) -> Result<(), String> {
    let held = hold_retirement_directory(path)?;
    resource_storage::check_components(path)?;
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if !kind.is_dir() || kind.is_symlink() {
            return Err(
                "Unexpected files or recovery data remain; inspect the storage folder".into(),
            );
        }
        prune_empty(&entry.path())?;
    }
    drop(held);
    fs::remove_dir(path).map_err(|e| e.to_string())
}
impl ManagedStorage for LocalStorage {
    fn retire_empty(&self, cask: &casklet_contracts::CaskView) -> Result<(), String> {
        uuid::Uuid::parse_str(&cask.id).map_err(|_| "Invalid Cask storage identity")?;
        let expected = self.root.join(&cask.id);
        let path = Path::new(&cask.managed_storage_root);
        resource_storage::check_components(path)?;
        if !path.exists() {
            return Ok(());
        }
        if path.canonicalize().map_err(|e| e.to_string())?
            != expected.canonicalize().map_err(|e| e.to_string())?
        {
            return Err("Storage location does not match this Cask".into());
        }
        let held_root = hold_retirement_directory(path)?;
        resource_storage::check_components(path)?;
        // Never recursively delete content: links, unknown bytes, staging, and quarantine
        // are legitimate reasons to retain storage for inspection.

        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name() == ".casklet" {
                continue;
            }
            prune_empty(&entry.path())?;
        }
        let metadata = path.join(".casklet");
        if metadata.exists() {
            resource_storage::check_components(&metadata)?;
            for entry in fs::read_dir(&metadata).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                resource_storage::check_components(&entry.path())?;
                if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                    prune_empty(&entry.path())?;
                } else if entry.file_name() != "manifest.json"
                    && entry.file_name() != "manifest.sha256"
                {
                    return Err(
                        "Recovery or unexpected metadata remain; inspect the storage folder".into(),
                    );
                }
            }
            let audit = self
                .root
                .parent()
                .ok_or("Storage parent unavailable")?
                .join("retired-casks");
            fs::create_dir_all(&audit).map_err(|e| e.to_string())?;
            resource_storage::check_components(&audit)?;
            fs::rename(&metadata, audit.join(&cask.id)).map_err(|e| e.to_string())?;
        }
        drop(held_root);
        fs::remove_dir(path).map_err(|e| e.to_string())
    }
    fn root_for(&self, id: &str) -> String {
        self.root.join(id).to_string_lossy().into_owned()
    }
    fn initialize(&self, cask: &Cask) -> Result<(), String> {
        let root = PathBuf::from(&cask.managed_storage_root);
        fs::create_dir_all(root.join("content")).map_err(|e| e.to_string())?;
        let metadata = root.join(".casklet");
        fs::create_dir_all(&metadata).map_err(|e| e.to_string())?;
        self.project(cask, &[], &[], &[])
    }
}

/// # Errors
/// Returns an I/O or migration error when development state cannot open.
pub fn development_service() -> Result<CaskService<SqliteCasks, LocalStorage>, String> {
    let root = std::env::var_os("CASKLET_DEV_ROOT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.casklet-dev"),
        PathBuf::from,
    );
    service_at(&root, root.join("casks"))
}

/// Opens the installed per-user profile, separate from development state.
/// # Errors
/// Returns an error if the user profile is unavailable or storage cannot open.
pub fn production_service() -> Result<CaskService<SqliteCasks, LocalStorage>, String> {
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or("LOCALAPPDATA is unavailable")?;
    #[cfg(windows)]
    let root = windows_data_root(&root);
    #[cfg(not(windows))]
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("HOME is unavailable")?
        .join(".local/share/Casklet");
    #[cfg(windows)]
    let casks = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or("USERPROFILE is unavailable")?
        .join("Casklets");
    #[cfg(not(windows))]
    let casks = root.join("casks");
    service_at(&root, casks)
}

#[cfg(windows)]
fn windows_data_root(local_app_data: &Path) -> PathBuf {
    local_app_data.join("CaskletData")
}

fn service_at(
    root: &Path,
    casks: PathBuf,
) -> Result<CaskService<SqliteCasks, LocalStorage>, String> {
    let repository = SqliteCasks::open(root.join("db/casklet.db"))?;
    repository.detect_abandoned_operations()?;
    repository.detect_abandoned_removals()?;
    let service = CaskService {
        repository,
        storage: LocalStorage::new(casks),
    };
    let _ = service.retry_manifest_projections("startup");
    Ok(service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::resource::{IntakePaths, ResourceRepository, ResourceStorage};
    use casklet_contracts::{
        AddMode, AddResource, BackupDatabase, CONTRACT_VERSION, CategorizeResource, CreateCask,
        CreateCategory, GetCask, GetResource, GetStorageSummary, ListCasks, ListResources,
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
    #[allow(clippy::too_many_lines)] // Integration fixture covers cross-layer persistence and sharing.
    fn library_deduplicates_shared_resources_and_metadata_survives_restart() {
        let root = std::env::temp_dir().join(format!("casklet-library-{}", uuid::Uuid::new_v4()));
        let db = root.join("db/casklet.db");
        let service = CaskService {
            repository: SqliteCasks::open(db.clone()).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let create = |name: &str| {
            service
                .create(
                    &CreateCask {
                        version: CONTRACT_VERSION,
                        name: name.into(),
                    },
                    "test",
                )
                .unwrap()
        };
        let first = create("First");
        let second = create("Second");
        let source = root.join("notes.txt");
        fs::write(&source, b"notes").unwrap();
        service
            .add_resource(
                &AddResource {
                    version: CONTRACT_VERSION,
                    cask_id: first.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    mode: AddMode::Copy,
                    category_key: "other".into(),
                },
                "test",
                &|_, _| {},
            )
            .unwrap();
        let resources = service
            .list_resources(
                &ListResources {
                    version: CONTRACT_VERSION,
                    cask_id: first.id.clone(),
                },
                "test",
            )
            .unwrap();
        Connection::open(&db).unwrap().execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,'other','REFERENCED','DETACH_ONLY',1)", params![uuid::Uuid::new_v4().to_string(),second.id,resources[0].id]).unwrap();
        let library = service
            .cask_library(
                &ListCasks {
                    version: CONTRACT_VERSION,
                },
                "test",
            )
            .unwrap();
        assert_eq!(library.total_items, 1);
        assert_eq!(library.managed_bytes, 5);
        assert!(library.casks.iter().all(|item| item.item_count == 1));
        assert!(
            service
                .list_resources(
                    &ListResources {
                        version: CONTRACT_VERSION,
                        cask_id: second.id.clone()
                    },
                    "test"
                )
                .unwrap()[0]
                .shared
        );
        let current = service
            .get(
                &GetCask {
                    version: CONTRACT_VERSION,
                    id: first.id.clone(),
                },
                "test",
            )
            .unwrap();
        let request = casklet_contracts::UpdateCask {
            version: CONTRACT_VERSION,
            id: first.id.clone(),
            name: "Renamed".into(),
            description: "Project notes".into(),
            archived: true,
            expected_revision: current.revision,
        };
        let updated = service.update_cask(&request, "test").unwrap();
        assert_eq!(updated.managed_storage_root, first.managed_storage_root);
        assert!(service.update_cask(&request, "test").is_err());
        let manifest = fs::read_to_string(
            Path::new(&first.managed_storage_root).join(".casklet/manifest.json"),
        )
        .unwrap();
        assert!(manifest.contains("Project notes"));
        drop(service);
        let reopened = SqliteCasks::open(db)
            .unwrap()
            .get(&first.id)
            .unwrap()
            .unwrap();
        assert_eq!(reopened.description, "Project notes");
        assert_eq!(reopened.state, casklet_domain::CaskState::Archived);
        assert_eq!(fs::read(source).unwrap(), b"notes");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn installed_program_directory_is_separate_from_operational_data() {
        let local_app_data = Path::new(r"C:\Users\tester\AppData\Local");
        let install_root = local_app_data.join("Casklet");
        let data_root = windows_data_root(local_app_data);
        assert_ne!(data_root, install_root);
        assert!(!data_root.starts_with(&install_root));
        assert_eq!(data_root, local_app_data.join("CaskletData"));
    }

    #[test]
    #[ignore = "Run explicitly in release mode on the supported Windows reference machine"]
    fn profile_thousand_resource_inventory_and_storage() {
        use std::time::Instant;
        let root = std::env::temp_dir().join(format!("casklet-profile-{}", uuid::Uuid::new_v4()));
        let db = root.join("db/casklet.db");
        let service = CaskService {
            repository: SqliteCasks::open(db.clone()).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Profile".into(),
                },
                "profile",
            )
            .unwrap();
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        let mut connection = Connection::open(&db).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        let transaction = connection.transaction().unwrap();
        for index in 0..1000 {
            let id = format!("resource-{index}");
            let path = source.join(format!("item-{index}.txt"));
            fs::write(&path, b"one").unwrap();
            let path_text = path.to_string_lossy().into_owned();
            transaction.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms,revision) VALUES (?1,'FILE',?2,'REFERENCED','USER_SELECTED','PRESENT',3,'EXACT',0,1,0)", params![id,format!("item-{index}.txt")]).unwrap();
            transaction.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms) VALUES (?1,?2,?3,?3,'EXTERNAL',1,1)", params![format!("location-{index}"),id,path_text]).unwrap();
            transaction.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,'other','REFERENCED','DETACH_ONLY',1)", params![format!("association-{index}"),cask.id,id]).unwrap();
        }
        transaction.commit().unwrap();
        drop(connection);
        let start = Instant::now();
        let inventory = service
            .list_resources(
                &ListResources {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                },
                "profile",
            )
            .unwrap();
        let inventory_elapsed = start.elapsed();
        let start = Instant::now();
        let summary = service
            .storage_summary(
                &GetStorageSummary {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                },
                "profile",
            )
            .unwrap();
        let summary_elapsed = start.elapsed();
        assert_eq!(inventory.len(), 1000);
        assert_eq!(summary.referenced_logical_bytes, 3000);
        println!(
            "inventory_1000_ms={} storage_summary_1000_ms={}",
            inventory_elapsed.as_millis(),
            summary_elapsed.as_millis()
        );
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn manual_backup_is_consistent_and_keeps_managed_files_separate() {
        let root = std::env::temp_dir().join(format!("casklet-backup-{}", uuid::Uuid::new_v4()));
        let service = CaskService {
            repository: SqliteCasks::open(root.join("db/casklet.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Backup test".into(),
                },
                "test",
            )
            .unwrap();
        let managed = PathBuf::from(&cask.managed_storage_root).join("content/keep.txt");
        fs::write(&managed, b"managed file stays outside the database backup").unwrap();
        let result = service
            .backup_database(
                &BackupDatabase {
                    version: CONTRACT_VERSION,
                },
                "test",
            )
            .unwrap();
        let backup_path = PathBuf::from(&result.path);
        assert!(backup_path.starts_with(root.join("backups")));
        assert_eq!(fs::metadata(&backup_path).unwrap().len(), result.size_bytes);
        let backup = Connection::open(&backup_path).unwrap();
        let name: String = backup
            .query_row("SELECT name FROM casks WHERE id=?1", [&cask.id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(name, "Backup test");
        assert_eq!(
            fs::read(managed).unwrap(),
            b"managed file stays outside the database backup"
        );
        drop(backup);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_prerelease_schema_upgrades_with_a_consistent_backup() {
        let migrations = [
            include_str!("../../../migrations/0001_casks.sql"),
            include_str!("../../../migrations/0002_resources.sql"),
            include_str!("../../../migrations/0003_operations.sql"),
            include_str!("../../../migrations/0004_removal_plans.sql"),
            include_str!("../../../migrations/0005_removal_execution.sql"),
            include_str!("../../../migrations/0006_permanent_disposal.sql"),
            include_str!("../../../migrations/0007_portable_installations.sql"),
            include_str!("../../../migrations/0008_windows_inventory.sql"),
            include_str!("../../../migrations/0009_winget_lifecycle.sql"),
            include_str!("../../../migrations/0010_installer_artifacts.sql"),
            include_str!("../../../migrations/0011_cask_metadata.sql"),
            include_str!("../../../migrations/0012_ownership_and_cleanup.sql"),
            include_str!("../../../migrations/0013_cask_removal.sql"),
            include_str!("../../../migrations/0014_managed_msi.sql"),
            include_str!("../../../migrations/0015_installer_provenance.sql"),
            include_str!("../../../migrations/0016_application_lifecycle.sql"),
        ];
        for version in 1..=16 {
            let root = std::env::temp_dir().join(format!(
                "casklet-upgrade-{version}-{}",
                uuid::Uuid::new_v4()
            ));
            let db = root.join("db/casklet.db");
            fs::create_dir_all(db.parent().unwrap()).unwrap();
            let connection = Connection::open(&db).unwrap();
            for sql in &migrations[..version] {
                connection.execute_batch(sql).unwrap();
            }
            let schema_version = i64::try_from(version).unwrap();
            connection
                .pragma_update(None, "user_version", schema_version)
                .unwrap();
            connection.execute("INSERT INTO casks(id,name,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state) VALUES ('existing','Existing','existing','C:/existing',1,1,0,'ACTIVE')", []).unwrap();
            drop(connection);
            let reopened = SqliteCasks::open(db).unwrap();
            assert_eq!(reopened.list().unwrap()[0].name, "Existing");
            assert!(reopened.list().unwrap()[0].description.is_empty());
            if version < 16 {
                let backups = fs::read_dir(root.join("backups"))
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                assert_eq!(backups.len(), 1);
                let backup = Connection::open(backups[0].path()).unwrap();
                let backed_up_version: i64 = backup
                    .query_row("PRAGMA user_version", [], |row| row.get(0))
                    .unwrap();
                assert_eq!(backed_up_version, schema_version);
            }
            drop(reopened);
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn newer_database_is_rejected_without_downgrading_or_changing_data() {
        let root = std::env::temp_dir().join(format!("casklet-newer-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = root.join("state.db");
        let connection = Connection::open(&db).unwrap();
        connection.execute_batch("CREATE TABLE preserved(value TEXT); INSERT INTO preserved VALUES ('user metadata');").unwrap();
        let future_version = MIGRATIONS.len() + 1;
        connection
            .pragma_update(None, "user_version", i64::try_from(future_version).unwrap())
            .unwrap();
        drop(connection);
        let original = fs::read(&db).unwrap();
        let error = SqliteCasks::open(db.clone()).unwrap_err();
        assert!(error.contains(&format!("schema version: {future_version}")));
        assert!(error.contains("matching or newer Casklet setup"));
        assert_eq!(fs::read(&db).unwrap(), original);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_database_is_preserved_and_rejected() {
        let root = std::env::temp_dir().join(format!("casklet-corrupt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = root.join("state.db");
        let bytes = b"not a SQLite database";
        fs::write(&db, bytes).unwrap();
        assert!(SqliteCasks::open(db.clone()).is_err());
        assert_eq!(fs::read(db).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "directory planning and both stale-plan boundaries"
    )]
    fn managed_directory_plan_detects_nested_change_before_approval() {
        let root = std::env::temp_dir().join(format!("casklet-m4-dir-{}", uuid::Uuid::new_v4()));
        let source = root.join("source-directory");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("nested").join("note.txt"), b"original").unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Directory".into(),
                },
                "test",
            )
            .unwrap();
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
        let permanent_plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![managed.id.clone()],
                    category_key: None,
                    permanent: true,
                },
                "test",
            )
            .unwrap();
        assert_eq!(permanent_plan.items[0].action, "PRESERVE");
        assert_eq!(
            permanent_plan.items[0].reason_code,
            "PERMANENT_DIRECTORY_UNSUPPORTED"
        );
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    resource_ids: vec![managed.id.clone()],
                    category_key: None,
                    permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(plan.items[0].action, "TRASH");
        let approved = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    resource_ids: vec![managed.id],
                    category_key: None,
                    permanent: false,
                },
                "test",
            )
            .unwrap();
        service
            .approve_removal_plan(
                &ApproveRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: approved.id.clone(),
                    plan_hash: approved.plan_hash.clone(),
                },
                "test",
            )
            .unwrap();
        fs::write(
            std::path::Path::new(&managed.path)
                .join("nested")
                .join("note.txt"),
            b"changed",
        )
        .unwrap();
        assert_eq!(
            service
                .approve_removal_plan(
                    &ApproveRemovalPlan {
                        version: CONTRACT_VERSION,
                        id: plan.id,
                        plan_hash: plan.plan_hash,
                    },
                    "test"
                )
                .unwrap_err()
                .code,
            "PLAN_CHANGED"
        );
        let result = service
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    version: CONTRACT_VERSION,
                    id: approved.id,
                    plan_hash: approved.plan_hash,
                    confirm_permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(
            result.items[0].result_code.as_deref(),
            Some("REVALIDATION_FAILED")
        );
        assert!(std::path::Path::new(&managed.path).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "full directory cleanup setup and journal assertion"
    )]
    fn approved_directory_cleanup_uses_journal_and_preserves_source() {
        struct FakeDirectoryTrash {
            storage: LocalStorage,
            stash: PathBuf,
        }
        impl casklet_application::removal::RemovalInspection for FakeDirectoryTrash {
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
                if candidate.kind != "DIRECTORY"
                    || self.verify_managed(root, candidate).as_deref() != Some(identity)
                {
                    return Err("directory changed".into());
                }
                fs::rename(&candidate.path, &self.stash).map_err(|e| e.to_string())
            }
        }
        let root =
            std::env::temp_dir().join(format!("casklet-m4-dir-run-{}", uuid::Uuid::new_v4()));
        let source = root.join("source-directory");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("nested").join("note.txt"), b"original").unwrap();
        let storage_root = root.join("casks");
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(storage_root.clone()),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Directory".into(),
                },
                "test",
            )
            .unwrap();
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
                    cask_id: cask.id,
                    resource_ids: vec![managed.id],
                    category_key: None,
                    permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(plan.items[0].action, "TRASH");
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
        let stash = root.join("fake-recycle-directory");
        let executor = CaskService {
            repository,
            storage: FakeDirectoryTrash {
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
                    confirm_permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.items[0].result_code.as_deref(), Some("TRASHED"));
        assert!(stash.join("nested").join("note.txt").exists());
        assert!(!std::path::Path::new(&managed.path).exists());
        assert!(source.join("nested").join("note.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn permanent_file_requires_opt_in_and_confirmation() {
        let root =
            std::env::temp_dir().join(format!("casklet-m4-permanent-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"keep source").unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Permanent".into(),
                },
                "test",
            )
            .unwrap();
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
        let external_link = root.join("external-hard-link.txt");
        fs::hard_link(&managed.path, &external_link).unwrap();
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    resource_ids: vec![managed.id],
                    category_key: None,
                    permanent: true,
                },
                "test",
            )
            .unwrap();
        assert_eq!(plan.items[0].action, "DELETE_PERMANENTLY");
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
        let request = ExecuteRemovalPlan {
            version: CONTRACT_VERSION,
            id: plan.id,
            plan_hash: plan.plan_hash,
            confirm_permanent: false,
        };
        assert_eq!(
            service
                .execute_removal_plan(&request, "test")
                .unwrap_err()
                .code,
            "PERMANENT_CONFIRMATION_REQUIRED"
        );
        assert!(std::path::Path::new(&managed.path).exists());
        let result = service
            .execute_removal_plan(
                &ExecuteRemovalPlan {
                    confirm_permanent: true,
                    ..request
                },
                "test",
            )
            .unwrap();
        assert_eq!(
            result.items[0].result_code.as_deref(),
            Some("PERMANENTLY_DELETED")
        );
        assert!(!std::path::Path::new(&managed.path).exists());
        assert!(source.exists());
        assert_eq!(fs::read(&external_link).unwrap(), b"keep source");
        fs::remove_dir_all(root).unwrap();
    }
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
        assert_eq!(schema_version, 16);
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
                    permanent: false,
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
                    permanent: false,
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
                    permanent: false,
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
                        category_key: None,
                        permanent: false,
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
                    permanent: false,
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
                    permanent: false,
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
                    confirm_permanent: false,
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
            CaskState::Active
        );
        drop(executor);
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "integration fixture checks replacement and hard-link swaps after approval"
    )]
    fn execution_preserves_replaced_file_and_hard_link_swap_after_approval() {
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
        for hard_link in [false, true] {
            let root =
                std::env::temp_dir().join(format!("casklet-m4-swap-{}", uuid::Uuid::new_v4()));
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
                        permanent: false,
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
            if hard_link {
                fs::hard_link(&source, &managed.path).unwrap();
            } else {
                fs::write(&managed.path, b"same bytes").unwrap();
            }
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
                        confirm_permanent: false,
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
                CaskState::Active
            );
            drop(executor);
            drop(service);
            fs::remove_dir_all(root).unwrap();
        }
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
                    permanent: false,
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
                    confirm_permanent: false,
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
                    permanent: false,
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
                    confirm_permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(result.state, "PARTIAL");
        assert_eq!(result.items[0].state, "UNKNOWN");
        assert_eq!(
            repository.get(&cask.id).unwrap().unwrap().state,
            CaskState::Active
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
        assert_eq!(closed.items[0].state, "UNKNOWN");
        assert_eq!(
            closed.items[0].result_code.as_deref(),
            Some("CLOSED_UNRESOLVED")
        );
        assert_eq!(
            repository.get(&cask.id).unwrap().unwrap().state,
            CaskState::Active
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
    fn removal_hard_exit_helper() {
        let Ok(root_text) = std::env::var("CASKLET_TEST_REMOVAL_EXIT_ROOT") else {
            return;
        };
        #[expect(
            clippy::items_after_statements,
            reason = "fault adapter is local to the helper"
        )]
        struct ExitAfterQuarantine(LocalStorage);
        #[expect(
            clippy::items_after_statements,
            reason = "fault adapter is local to the helper"
        )]
        impl casklet_application::removal::RemovalInspection for ExitAfterQuarantine {
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
                root: &str,
                candidate: &casklet_application::removal::RemovalCandidate,
                identity: &str,
                run_id: &str,
                item_id: &str,
            ) -> Result<(), String> {
                assert_eq!(
                    self.verify_managed(root, candidate).as_deref(),
                    Some(identity)
                );
                let quarantine = std::path::Path::new(root)
                    .join(".casklet")
                    .join("removal-staging")
                    .join(run_id)
                    .join(item_id);
                fs::create_dir_all(quarantine.parent().unwrap()).unwrap();
                fs::rename(&candidate.path, &quarantine).unwrap();
                std::process::exit(74);
            }
        }
        let root = PathBuf::from(root_text);
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: ExitAfterQuarantine(LocalStorage::new(root.join("casks"))),
        };
        let plan_id = std::env::var("CASKLET_TEST_REMOVAL_EXIT_PLAN").unwrap();
        let hash = std::env::var("CASKLET_TEST_REMOVAL_EXIT_HASH").unwrap();
        let _ = service.execute_removal_plan(
            &ExecuteRemovalPlan {
                version: CONTRACT_VERSION,
                id: plan_id,
                plan_hash: hash,
                confirm_permanent: false,
            },
            "test",
        );
        panic!("removal fault point was not reached");
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "child-process fault test verifies the real durable boundary after quarantine"
    )]
    fn hard_exit_after_quarantine_requires_preserved_recovery() {
        let root = std::env::temp_dir().join(format!("casklet-m4-exit-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Crash".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"crash-safe").unwrap();
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
                    permanent: false,
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
        drop(service);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "tests::removal_hard_exit_helper", "--nocapture"])
            .env("CASKLET_TEST_REMOVAL_EXIT_ROOT", &root)
            .env("CASKLET_TEST_REMOVAL_EXIT_PLAN", &plan.id)
            .env("CASKLET_TEST_REMOVAL_EXIT_HASH", &plan.plan_hash)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(74),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let repository = SqliteCasks::open(root.join("state.db")).unwrap();
        repository
            .detect_abandoned_removals_with(|_| false)
            .unwrap();
        let recovered =
            casklet_application::removal::RemovalRepository::get_plan(&repository, &plan.id)
                .unwrap()
                .unwrap();
        assert_eq!(recovered.state, "PARTIAL");
        assert_eq!(recovered.items[0].state, "UNKNOWN");
        let quarantine = std::path::Path::new(&cask.managed_storage_root)
            .join(".casklet")
            .join("removal-staging")
            .join(recovered.run_id.as_deref().unwrap())
            .join(&recovered.items[0].id);
        assert!(quarantine.exists());
        assert!(!std::path::Path::new(&managed.path).exists());
        assert!(source.exists());
        let service = CaskService {
            repository,
            storage: LocalStorage::new(root.join("casks")),
        };
        let closed = service
            .close_uncertain_removal(
                &CloseRemovalRun {
                    version: CONTRACT_VERSION,
                    run_id: recovered.run_id.unwrap(),
                },
                "test",
            )
            .unwrap();
        assert_eq!(closed.items[0].state, "UNKNOWN");
        assert_eq!(
            closed.items[0].result_code.as_deref(),
            Some("CLOSED_UNRESOLVED")
        );
        assert_eq!(
            closed.items[0].recovery_path.as_deref(),
            Some(quarantine.to_string_lossy().as_ref())
        );
        assert!(quarantine.exists());
        fs::rename(&quarantine, &managed.path).unwrap();
        let retry = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    resource_ids: vec![managed.id],
                    category_key: None,
                    permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(retry.items[0].action, "TRASH");
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
