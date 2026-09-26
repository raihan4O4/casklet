//! Durable operation queries and restart reconciliation.
use super::SqliteCasks;
use casklet_application::operation::{
    IntakeEvidence, OperationRepository, PendingIntake, RecoveryFilesystem, RegisteredIntake,
};
use casklet_application::resource::IntakePaths;
use casklet_contracts::{CONTRACT_VERSION, OperationView};
use casklet_domain::resource::{IntakeMode, Resource, ResourceKind};
use rusqlite::OptionalExtension;
use std::{fs, path::Path, process::Command};

fn operation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<OperationView> {
    let completed: i64 = row.get(4)?;
    let total: Option<i64> = row.get(5)?;
    Ok(OperationView {
        version: CONTRACT_VERSION,
        id: row.get(0)?,
        cask_id: row.get(1)?,
        state: row.get(2)?,
        step: row.get(3)?,
        progress_completed: u64::try_from(completed).unwrap_or(0),
        progress_total: total.and_then(|n| u64::try_from(n).ok()),
        cancellation_requested: row.get::<_, i64>(6)? == 1,
        error_code: row.get(7)?,
        updated_at_ms: row.get(8)?,
    })
}

const SELECT_OPERATION: &str = "SELECT id,cask_id,state,current_step_code,progress_completed,progress_total,cancellation_requested,error_code,updated_at_ms FROM operations";

impl OperationRepository for SqliteCasks {
    fn list_operations(&self, cask_id: Option<&str>) -> Result<Vec<OperationView>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(&format!(
                "{SELECT_OPERATION} WHERE (?1 IS NULL OR cask_id=?1) ORDER BY started_at_ms DESC,id"
            ))
            .map_err(|e| e.to_string())?;
        statement
            .query_map([cask_id], operation_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn get_operation(&self, id: &str) -> Result<Option<OperationView>, String> {
        self.connection()?
            .query_row(
                &format!("{SELECT_OPERATION} WHERE id=?1"),
                [id],
                operation_row,
            )
            .optional()
            .map_err(|e| e.to_string())
    }

    fn pending_manifests(&self) -> Result<Vec<String>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT cask_id FROM projection_jobs WHERE state='PENDING' ORDER BY updated_at_ms",
            )
            .map_err(|e| e.to_string())?;
        statement
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<String>, _>>()
            .map_err(|e| e.to_string())
    }

    fn registered_intake(&self, id: &str) -> Result<Option<RegisteredIntake>, String> {
        let candidate: Option<(String, String, Option<i64>)> = self.connection()?.query_row(
            "SELECT i.final_path,r.kind,r.size_bytes FROM intake_operations i JOIN resources r ON r.id=i.resource_id WHERE i.id=?1 AND i.state='REGISTERED'",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional().map_err(|e| e.to_string())?;
        candidate
            .map(|(path, kind, size)| {
                Ok(RegisteredIntake {
                    path,
                    kind: ResourceKind::from_code(&kind).ok_or("Invalid recorded resource kind")?,
                    size_bytes: size
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|e| e.to_string())?,
                })
            })
            .transpose()
    }

    fn finish_registered_intake(&self, id: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE intake_operations SET state='COMPLETE' WHERE id=?1 AND state='REGISTERED'",
                [id],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Intake registration changed during recovery".into());
        }
        transaction.execute("UPDATE portable_install_intents SET state='COMPLETE' WHERE operation_id=?1 AND state='REGISTERED'",[id]).map_err(|e|e.to_string())?;
        let changed = transaction.execute("UPDATE operations SET state='SUCCEEDED',current_step_code='COMPLETE',error_code=NULL,finished_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER),revision=revision+1 WHERE id=?1 AND state='RECOVERY_REQUIRED'",[id]).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Operation state changed during recovery".into());
        }
        transaction
            .execute("DELETE FROM operation_locks WHERE operation_id=?1", [id])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,'INTAKE_RECOVERED',CAST(unixepoch('subsec')*1000 AS INTEGER))",[id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn preserve_operation(&self, id: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let owner: Option<String> = connection
            .query_row(
                "SELECT owner_instance_id FROM operation_locks WHERE operation_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if owner.as_deref().is_some_and(|owner| {
            owner != std::process::id().to_string() && owner_process_alive(owner)
        }) {
            return Err("The operation owner may still be running".into());
        }
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let current_owner: Option<String> = transaction
            .query_row(
                "SELECT owner_instance_id FROM operation_locks WHERE operation_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if current_owner != owner {
            return Err("Operation lock changed during recovery".into());
        }
        let changed = transaction.execute("UPDATE operations SET state='CLOSED_PRESERVED',current_step_code='PRESERVED',finished_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER),revision=revision+1 WHERE id=?1 AND state='RECOVERY_REQUIRED'",[id]).map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Operation is not awaiting recovery".into());
        }
        transaction
            .execute("DELETE FROM operation_locks WHERE operation_id=?1", [id])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,'FILES_PRESERVED',CAST(unixepoch('subsec')*1000 AS INTEGER))",[id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn intake_evidence(&self, id: &str) -> Result<Option<IntakeEvidence>, String> {
        self.connection()?.query_row("SELECT CASE WHEN EXISTS(SELECT 1 FROM portable_install_intents p WHERE p.operation_id=i.id) THEN 'PORTABLE_ZIP' ELSE i.mode END,i.source_path,i.staging_path,i.final_path FROM intake_operations i WHERE i.id=?1",[id],|row| Ok(IntakeEvidence { mode:row.get(0)?,source_path:row.get(1)?,staging_path:row.get(2)?,final_path:row.get(3)? }))
            .optional().map_err(|e|e.to_string())
    }
    fn request_cancellation(&self, id: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction.execute("UPDATE operations SET cancellation_requested=1,revision=revision+1 WHERE id=?1 AND state='RUNNING'",[id]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Operation is not running".into());
        }
        transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,'CANCELLATION_REQUESTED',CAST(unixepoch('subsec')*1000 AS INTEGER))",[id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }
    fn pending_intake(&self, id: &str) -> Result<Option<PendingIntake>, String> {
        type Row = (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<i64>,
            Option<i64>,
            i64,
            String,
            String,
        );
        let row: Option<Row> = self.connection()?.query_row(
            "SELECT i.resource_id,i.cask_id,i.mode,i.source_path,i.staging_path,i.final_path,i.resource_name,i.size_bytes,i.unknown_entries,i.created_at_ms,i.resource_kind,i.category_key FROM intake_operations i JOIN operations o ON o.id=i.id WHERE i.id=?1 AND i.state IN ('PLANNED','RECOVERY_REQUIRED') AND o.state='RECOVERY_REQUIRED' AND NOT EXISTS(SELECT 1 FROM portable_install_intents p WHERE p.operation_id=i.id) AND i.resource_name IS NOT NULL AND i.resource_kind IS NOT NULL AND i.category_key IS NOT NULL",
            [id],|row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?))
        ).optional().map_err(|e|e.to_string())?;
        row.map(
            |(
                resource_id,
                cask_id,
                mode,
                source,
                staging,
                final_path,
                name,
                size,
                unknown,
                created,
                kind,
                category,
            )| {
                let mode = match mode.as_str() {
                    "COPY" => IntakeMode::Copy,
                    "MOVE" => IntakeMode::Move,
                    _ => return Err("Invalid recorded intake mode".into()),
                };
                let kind =
                    ResourceKind::from_code(&kind).ok_or("Invalid recorded resource kind")?;
                let size = size
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|e| e.to_string())?;
                let unknown = u64::try_from(unknown.ok_or("Missing recorded unknown count")?)
                    .map_err(|e| e.to_string())?;
                if size.is_none() || unknown != 0 {
                    return Err("Intake snapshot cannot be safely verified".into());
                }
                let mut resource = Resource::new(
                    cask_id,
                    name,
                    source.clone(),
                    kind,
                    mode,
                    category,
                    size,
                    unknown,
                    created,
                )
                .map_err(|e| format!("Invalid intake snapshot: {e:?}"))?;
                resource.id = resource_id;
                resource.path.clone_from(&final_path);
                Ok(PendingIntake {
                    resource,
                    mode,
                    source_path: source,
                    paths: IntakePaths {
                        staging,
                        final_path,
                    },
                })
            },
        )
        .transpose()
    }
}

impl RecoveryFilesystem for super::LocalStorage {
    fn path_status(&self, path: &str) -> String {
        let path = Path::new(path);
        if super::resource_storage::check_components(path).is_err() {
            return "UNVERIFIED_LINK".into();
        }
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => "PRESENT_FILE".into(),
            Ok(metadata) if metadata.is_dir() => "PRESENT_DIRECTORY".into(),
            Ok(_) => "UNSUPPORTED".into(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "MISSING".into(),
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                "INACCESSIBLE".into()
            }
            Err(_) => "UNKNOWN".into(),
        }
    }
}

impl SqliteCasks {
    /// Detects work whose owning process is gone, retaining all candidate files and locks.
    /// # Errors
    /// Returns a persistence error.
    pub fn detect_abandoned_operations(&self) -> Result<(), String> {
        let mut connection = self.connection()?;
        let mut statement = connection.prepare("SELECT o.id,l.owner_instance_id FROM operations o LEFT JOIN operation_locks l ON l.operation_id=o.id WHERE o.state='RUNNING'")
            .map_err(|e| e.to_string())?;
        let candidates = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(statement);
        for (id, owner) in candidates {
            if owner.as_deref().is_some_and(owner_process_alive) {
                continue;
            }
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            let changed = transaction.execute("UPDATE operations SET state='RECOVERY_REQUIRED',current_step_code='RECONCILE',error_code='INTERRUPTED',revision=revision+1 WHERE id=?1 AND state='RUNNING'",[&id]).map_err(|e| e.to_string())?;
            if changed == 1 {
                transaction.execute("UPDATE operation_steps SET state='RECOVERY_REQUIRED' WHERE operation_id=?1 AND state='RUNNING'",[&id]).map_err(|e| e.to_string())?;
                transaction.execute("UPDATE intake_operations SET state='RECOVERY_REQUIRED' WHERE id=?1 AND state='PLANNED'",[&id]).map_err(|e| e.to_string())?;
                transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,'INTAKE_INTERRUPTED',CAST(unixepoch('subsec')*1000 AS INTEGER))",[&id]).map_err(|e| e.to_string())?;
            }
            transaction.commit().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

pub(super) fn owner_process_alive(owner: &str) -> bool {
    let Ok(pid) = owner.parse::<u32>() else {
        return true;
    };
    #[cfg(windows)]
    {
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'ALIVE' }} else {{ 'DEAD' }}")])
            .output();
        let Ok(output) = output else {
            return true;
        };
        if !output.status.success() {
            return true;
        }
        String::from_utf8_lossy(&output.stdout).trim() != "DEAD"
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("true");
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}
