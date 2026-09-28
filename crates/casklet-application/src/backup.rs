//! Explicit database backup through a persistence capability.
use crate::{CaskService, api_error, check_version};
use casklet_contracts::{ApiError, BackupDatabase, DatabaseBackupView};

pub trait DatabaseBackupRepository {
    /// # Errors
    /// Returns a storage or integrity error without replacing the source database.
    fn backup_database(&self) -> Result<(String, u64), String>;
}

impl<R: DatabaseBackupRepository, S> CaskService<R, S> {
    /// Creates a consistent metadata-only backup in the application-owned backup directory.
    /// # Errors
    /// Returns a version, integrity, or storage error.
    pub fn backup_database(
        &self,
        request: &BackupDatabase,
        correlation_id: &str,
    ) -> Result<DatabaseBackupView, ApiError> {
        check_version(request.version, correlation_id)?;
        let (path, size_bytes) = self
            .repository
            .backup_database()
            .map_err(|error| api_error("DATABASE_BACKUP", error, correlation_id))?;
        Ok(DatabaseBackupView {
            version: casklet_contracts::CONTRACT_VERSION,
            path,
            size_bytes,
        })
    }
}
