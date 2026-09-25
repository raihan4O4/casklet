//! SQLite resource, category, and intake-intent persistence.
use super::SqliteCasks;
use casklet_application::resource::ResourceRepository;
use casklet_contracts::{CONTRACT_VERSION, CategoryView, ResourceView};
use casklet_domain::resource::{IntakeMode, Resource};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

const SELECT_RESOURCE: &str = "SELECT r.id,a.cask_id,r.display_name,l.path_display,r.kind,r.management,l.containment,r.availability,a.category_key,r.size_bytes,r.size_measurement,r.unknown_entries FROM resources r JOIN resource_associations a ON a.resource_id=r.id AND a.removed_at_ms IS NULL JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL";

fn resource_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResourceView> {
    let size: Option<i64> = row.get(9)?;
    let unknown: i64 = row.get(11)?;
    Ok(ResourceView {
        version: CONTRACT_VERSION,
        id: row.get(0)?,
        cask_id: row.get(1)?,
        name: row.get(2)?,
        path: row.get(3)?,
        kind: row.get(4)?,
        management: row.get(5)?,
        containment: row.get(6)?,
        availability: row.get(7)?,
        category_key: row.get(8)?,
        size_bytes: size.and_then(|n| u64::try_from(n).ok()),
        size_measurement: row.get(10)?,
        unknown_entries: u64::try_from(unknown).unwrap_or(0),
    })
}

impl ResourceRepository for SqliteCasks {
    fn category_exists(&self, cask_id: &str, key: &str) -> Result<bool, String> {
        let count: i64 = self
            .connection()?
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE key=?1 AND (cask_id IS NULL OR cask_id=?2)",
                params![key, cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(count > 0)
    }
    fn create_category(&self, cask_id: &str, key: &str, name: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO categories(key,cask_id,display_name,is_builtin,sort_order) VALUES (?1,?2,?3,0,100)",
            params![key,cask_id,name]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',CAST(unixepoch('subsec')*1000 AS INTEGER)) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING'",[cask_id]).map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn list_categories(&self, cask_id: &str) -> Result<Vec<CategoryView>, String> {
        let connection = self.connection()?;
        let mut stmt = connection.prepare("SELECT key,display_name,is_builtin FROM categories WHERE cask_id IS NULL OR cask_id=?1 ORDER BY sort_order,display_name")
            .map_err(|e| e.to_string())?;
        stmt.query_map([cask_id], |row| {
            Ok(CategoryView {
                version: CONTRACT_VERSION,
                key: row.get(0)?,
                name: row.get(1)?,
                is_builtin: row.get::<_, i64>(2)? == 1,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
    fn begin_intake(
        &self,
        op_id: &str,
        resource: &Resource,
        mode: IntakeMode,
        source: &str,
        staging: &str,
        final_path: &str,
    ) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let removal_lock: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if removal_lock != 0 {
            return Err("Cask has an active or unrecovered removal".into());
        }
        transaction.execute("INSERT INTO operations(id,cask_id,type,state,current_step_code,started_at_ms,updated_at_ms) VALUES (?1,?2,'RESOURCE_INTAKE','RUNNING','MATERIALIZE',?3,?3)",
            params![op_id,resource.cask_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO operation_steps(operation_id,sequence,step_code,state,attempt_count,started_at_ms) VALUES (?1,1,'MATERIALIZE','RUNNING',1,?2)",
            params![op_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO operation_steps(operation_id,sequence,step_code,state,attempt_count) VALUES (?1,2,'REGISTER','PENDING',0)",[op_id]).map_err(|e|e.to_string())?;
        transaction.execute("INSERT INTO operation_locks(lock_key,operation_id,owner_instance_id,acquired_at_ms,heartbeat_at_ms) VALUES (?1,?2,?3,?4,?4)",
            params![format!("cask:{}:mutation",resource.cask_id),op_id,std::process::id().to_string(),resource.created_at_ms])
            .map_err(|e| format!("Cask has an active or unrecovered mutation: {e}"))?;
        transaction.execute("INSERT INTO intake_operations(id,cask_id,resource_id,mode,source_path,staging_path,final_path,state,created_at_ms,updated_at_ms,resource_name,resource_kind,category_key,size_bytes,unknown_entries) VALUES (?1,?2,?3,?4,?5,?6,?7,'PLANNED',?8,?8,?9,?10,?11,?12,?13)",
            params![op_id,resource.cask_id,resource.id,mode.code(),source,staging,final_path,resource.created_at_ms,resource.name,resource.kind.code(),resource.category_key,resource.size_bytes.and_then(|n| i64::try_from(n).ok()),i64::try_from(resource.unknown_entries).map_err(|e| e.to_string())?])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,'INTAKE_STARTED',?2)",params![op_id,resource.created_at_ms]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn commit_resource(&self, resource: &Resource, op_id: Option<&str>) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let locked: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if locked != 0 {
            return Err("Cask has an active or unrecovered removal".into());
        }
        let size = resource
            .size_bytes
            .map(i64::try_from)
            .transpose()
            .map_err(|e| e.to_string())?;
        let unknown = i64::try_from(resource.unknown_entries).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms) VALUES (?1,?2,?3,?4,?5,'PRESENT',?6,?7,?8,?9)",
            params![resource.id,resource.kind.code(),resource.name,resource.management.code(),
                if op_id.is_some() {"ADDED_BY_CASKLET"} else {"USER_REFERENCE"},size,resource.size_measurement,unknown,resource.created_at_ms])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms,identity_key) VALUES (?1,?2,?3,?4,?5,1,?6,?7)",
            params![Uuid::new_v4().to_string(),resource.id,resource.path,resource.path.to_lowercase(),resource.containment(),resource.created_at_ms,resource.identity_key])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![Uuid::new_v4().to_string(),resource.cask_id,resource.id,resource.category_key,resource.relationship(),
                if op_id.is_some() {"DELETE_WHEN_UNREFERENCED"} else {"DETACH_ONLY"},resource.created_at_ms])
            .map_err(|e| e.to_string())?;
        if let Some(op_id) = op_id {
            let changed = transaction.execute("UPDATE intake_operations SET state='REGISTERED',updated_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE id=?1 AND state IN ('PLANNED','RECOVERY_REQUIRED')",
                [op_id]).map_err(|e| e.to_string())?;
            if changed != 1 {
                return Err("Intake state changed during registration".into());
            }
            let changed = transaction.execute("UPDATE operation_steps SET state='SUCCEEDED',finished_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE operation_id=?1 AND sequence=2 AND state='RUNNING'",
                [op_id]).map_err(|e|e.to_string())?;
            if changed != 1 {
                return Err("Registration step changed".into());
            }
            transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",
                params![resource.cask_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        } else {
            transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",
                params![resource.cask_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }
    fn complete_intake(&self, op_id: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction
            .execute(
                "UPDATE intake_operations SET state='COMPLETE' WHERE id=?1 AND state='REGISTERED'",
                [op_id],
            )
            .map_err(|e| e.to_string())?;
        if changed != 1 {
            return Err("Intake was not registered".into());
        }
        transaction.execute("UPDATE operations SET state='SUCCEEDED',current_step_code='COMPLETE',finished_at_ms=updated_at_ms,revision=revision+1 WHERE id=?1 AND state IN ('RUNNING','RECOVERY_REQUIRED')",[op_id]).map_err(|e| e.to_string())?;
        transaction
            .execute("DELETE FROM operation_locks WHERE operation_id=?1", [op_id])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,'INTAKE_SUCCEEDED',CAST(unixepoch('subsec')*1000 AS INTEGER))",[op_id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn progress_intake(&self, op_id: &str, completed: u64, total: u64) -> Result<(), String> {
        let completed = i64::try_from(completed).map_err(|e| e.to_string())?;
        let total = i64::try_from(total).map_err(|e| e.to_string())?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction.execute("UPDATE operations SET progress_completed=?2,progress_total=?3,updated_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER),revision=revision+1 WHERE id=?1 AND state='RUNNING'",
            params![op_id,completed,total]).map_err(|e| e.to_string())?;
        transaction.execute("UPDATE operation_locks SET heartbeat_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE operation_id=?1",[op_id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn fail_intake(&self, op_id: &str, error_code: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction.execute("UPDATE intake_operations SET state='RECOVERY_REQUIRED' WHERE id=?1 AND state='PLANNED'",[op_id]).map_err(|e| e.to_string())?;
        transaction.execute("UPDATE operations SET state='RECOVERY_REQUIRED',current_step_code='RECONCILE',error_code=?2,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![op_id,error_code]).map_err(|e| e.to_string())?;
        transaction.execute("UPDATE operation_steps SET state='RECOVERY_REQUIRED' WHERE operation_id=?1 AND state='RUNNING'",[op_id]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO diagnostic_events(operation_id,event_code,at_ms) VALUES (?1,?2,CAST(unixepoch('subsec')*1000 AS INTEGER))",params![op_id,error_code]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn materialized_intake(&self, op_id: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction.execute("UPDATE operation_steps SET state='SUCCEEDED',finished_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE operation_id=?1 AND sequence=1 AND state IN ('RUNNING','RECOVERY_REQUIRED')",[op_id]).map_err(|e|e.to_string())?;
        if changed == 0 {
            let state: String = transaction
                .query_row(
                    "SELECT state FROM operation_steps WHERE operation_id=?1 AND sequence=1",
                    [op_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if state != "SUCCEEDED" {
                return Err("Materialization step changed".into());
            }
        }
        let changed = transaction.execute("UPDATE operation_steps SET state='RUNNING',attempt_count=attempt_count+1,started_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE operation_id=?1 AND sequence=2 AND state IN ('PENDING','RECOVERY_REQUIRED')",[op_id]).map_err(|e|e.to_string())?;
        if changed == 0 {
            let state: String = transaction
                .query_row(
                    "SELECT state FROM operation_steps WHERE operation_id=?1 AND sequence=2",
                    [op_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if state != "RUNNING" {
                return Err("Registration step changed".into());
            }
        }
        transaction.execute("UPDATE operations SET current_step_code='REGISTER',revision=revision+1 WHERE id=?1 AND state IN ('RUNNING','RECOVERY_REQUIRED')",[op_id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }
    fn cancellation_requested(&self, op_id: &str) -> Result<bool, String> {
        self.connection()?
            .query_row(
                "SELECT cancellation_requested FROM operations WHERE id=?1",
                [op_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|flag| flag == 1)
            .map_err(|e| e.to_string())
    }
    fn list_resources(&self, cask_id: &str) -> Result<Vec<ResourceView>, String> {
        let connection = self.connection()?;
        let mut stmt = connection
            .prepare(&format!(
                "{SELECT_RESOURCE} WHERE a.cask_id=?1 ORDER BY a.added_at_ms,r.id"
            ))
            .map_err(|e| e.to_string())?;
        stmt.query_map([cask_id], resource_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    fn get_resource(&self, id: &str) -> Result<Option<ResourceView>, String> {
        self.connection()?
            .query_row(
                &format!("{SELECT_RESOURCE} WHERE r.id=?1"),
                [id],
                resource_row,
            )
            .optional()
            .map_err(|e| e.to_string())
    }
    fn categorize(&self, id: &str, key: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let locked: i64 = transaction.query_row("SELECT COUNT(*) FROM resource_associations a JOIN removal_locks l ON l.cask_id=a.cask_id WHERE a.resource_id=?1 AND a.removed_at_ms IS NULL",[id],|row|row.get(0)).map_err(|e|e.to_string())?;
        if locked != 0 {
            return Err("Cask has an active or unrecovered removal".into());
        }
        let cask_id: String = transaction.query_row("SELECT cask_id FROM resource_associations WHERE resource_id=?1 AND removed_at_ms IS NULL",[id],|row|row.get(0)).map_err(|e|e.to_string())?;
        transaction.execute("UPDATE resource_associations SET category_key=?2,revision=revision+1 WHERE resource_id=?1 AND removed_at_ms IS NULL", params![id,key]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',CAST(unixepoch('subsec')*1000 AS INTEGER)) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING'",[cask_id]).map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(())
    }
}
