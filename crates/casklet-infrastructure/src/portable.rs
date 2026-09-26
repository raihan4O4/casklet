//! SQLite persistence for portable installations and their durable intake intent.
use super::SqliteCasks;
use casklet_application::portable::{PortablePaths, PortableRecord, PortableRepository};
use casklet_contracts::{CONTRACT_VERSION, InstallPortableZip, PortableInstallationView};
use casklet_domain::{Cask, portable::PortableIdentity, resource::Resource};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

const SELECT_PORTABLE: &str = "SELECT a.id,i.id,ci.cask_id,i.resource_id,a.canonical_name,i.version,i.provider,i.provenance,ci.relationship,i.state,root.path_display,i.executable_relative_path,i.source_sha256,i.executable_sha256,rl.identity_key,CASE WHEN ci.removed_at_ms IS NULL AND c.state='ACTIVE' AND i.state='INSTALLED' THEN 1 ELSE 0 END FROM installations i JOIN applications a ON a.id=i.application_id JOIN cask_installations ci ON ci.installation_id=i.id JOIN casks c ON c.id=ci.cask_id JOIN installation_locations root ON root.installation_id=i.id AND root.role='ROOT' LEFT JOIN resource_locations rl ON rl.resource_id=i.resource_id AND rl.is_primary=1 AND rl.retired_at_ms IS NULL";

fn row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PortableRecord> {
    let enabled = row.get::<_, i64>(15)? == 1;
    Ok(PortableRecord {
        view: PortableInstallationView {
            version: CONTRACT_VERSION,
            application_id: row.get(0)?,
            installation_id: row.get(1)?,
            cask_id: row.get(2)?,
            resource_id: row.get(3)?,
            application_name: row.get(4)?,
            application_version: row.get(5)?,
            provider: row.get(6)?,
            provenance: row.get(7)?,
            relationship: row.get(8)?,
            state: row.get(9)?,
            root_path: row.get(10)?,
            executable_relative_path: row.get(11)?,
            source_sha256: row.get(12)?,
            can_launch: enabled,
            can_plan_removal: enabled,
        },
        executable_sha256: row.get(13)?,
        root_identity: row.get::<_, Option<String>>(14)?.unwrap_or_default(),
    })
}

impl PortableRepository for SqliteCasks {
    fn begin_portable(
        &self,
        op_id: &str,
        cask: &Cask,
        ids: &PortableIdentity,
        resource: &Resource,
        request: &InstallPortableZip,
        paths: &PortablePaths,
        now: i64,
    ) -> Result<(), String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        let state: String = tx
            .query_row("SELECT state FROM casks WHERE id=?1", [&cask.id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if state != "ACTIVE" {
            return Err("Cask is not active".into());
        }
        let removing: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&cask.id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if removing != 0 {
            return Err("Cask has an active removal".into());
        }
        tx.execute("INSERT INTO operations(id,cask_id,type,state,current_step_code,started_at_ms,updated_at_ms,progress_total) VALUES (?1,?2,'RESOURCE_INTAKE','RUNNING','EXTRACT',?3,?3,?4)",
            params![op_id,cask.id,now,resource.size_bytes.and_then(|n|i64::try_from(n).ok())]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO operation_steps(operation_id,sequence,step_code,state,attempt_count,started_at_ms) VALUES (?1,1,'EXTRACT','RUNNING',1,?2)",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO operation_steps(operation_id,sequence,step_code,state,attempt_count) VALUES (?1,2,'REGISTER','PENDING',0)",[op_id]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO operation_locks(lock_key,operation_id,owner_instance_id,acquired_at_ms,heartbeat_at_ms) VALUES (?1,?2,?3,?4,?4)",
            params![format!("cask:{}:mutation",cask.id),op_id,std::process::id().to_string(),now]).map_err(|e|format!("Cask has an active mutation: {e}"))?;
        tx.execute("INSERT INTO intake_operations(id,cask_id,resource_id,mode,source_path,staging_path,final_path,state,created_at_ms,updated_at_ms,resource_name,resource_kind,category_key,size_bytes,unknown_entries) VALUES (?1,?2,?3,'COPY',?4,?5,?6,'PLANNED',?7,?7,?8,'DIRECTORY','applications',?9,0)",
            params![op_id,cask.id,resource.id,request.archive_path,paths.staging,paths.final_path,now,resource.name,resource.size_bytes.and_then(|n|i64::try_from(n).ok())]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO portable_install_intents(operation_id,cask_id,installation_id,resource_id,archive_path,archive_sha256,executable_relative_path,staging_path,final_path,state) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'PLANNED')",
            params![op_id,cask.id,ids.installation_id,resource.id,request.archive_path,request.archive_sha256,request.executable_relative_path,paths.staging,paths.final_path]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    fn commit_portable(
        &self,
        op_id: &str,
        cask: &Cask,
        ids: &PortableIdentity,
        resource: &Resource,
        request: &InstallPortableZip,
        executable_hash: &str,
        now: i64,
    ) -> Result<(), String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        let (stored_hash,stored_path,stored_exe,stored_resource,state):(String,String,String,String,String)=tx.query_row(
            "SELECT archive_sha256,final_path,executable_relative_path,resource_id,state FROM portable_install_intents WHERE operation_id=?1",
            [op_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(|e|e.to_string())?;
        if state != "PLANNED"
            || stored_hash != request.archive_sha256
            || stored_path != resource.path
            || stored_exe != request.executable_relative_path
            || stored_resource != resource.id
            || resource.identity_key.is_none()
        {
            return Err("Portable intent changed or lacks verified identity".into());
        }
        let active: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM operation_locks WHERE operation_id=?1",
                [op_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if active != 1 {
            return Err("Portable operation lock is missing".into());
        }
        tx.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms) VALUES (?1,'DIRECTORY',?2,'MANAGED','ADDED_BY_CASKLET','PRESENT',?3,'CALCULATED',0,?4)",
            params![resource.id,resource.name,resource.size_bytes.and_then(|n|i64::try_from(n).ok()),resource.created_at_ms]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms,identity_key) VALUES (?1,?2,?3,?4,'INTERNAL',1,?5,?6)",
            params![Uuid::new_v4().to_string(),resource.id,resource.path,resource.path.to_lowercase(),now,resource.identity_key]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,'applications','OWNED','DELETE_WHEN_UNREFERENCED',?4)",
            params![Uuid::new_v4().to_string(),cask.id,resource.id,now]).map_err(|e|e.to_string())?;
        tx.execute(
            "INSERT INTO applications(id,canonical_name,created_at_ms) VALUES (?1,?2,?3)",
            params![ids.application_id, resource.name, now],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO installations(id,application_id,provider,version,scope,provenance,state,source_sha256,executable_sha256,executable_relative_path,resource_id,installed_at_ms,last_verified_at_ms) VALUES (?1,?2,'PORTABLE_ZIP',?3,'CASK','INSTALLED_BY_CASKLET','INSTALLED',?4,?5,?6,?7,?8,?8)",
            params![ids.installation_id,ids.application_id,request.application_version,request.archive_sha256,executable_hash,request.executable_relative_path,resource.id,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO installation_locations(id,installation_id,path_display,role,last_verified_at_ms) VALUES (?1,?2,?3,'ROOT',?4)",
            params![Uuid::new_v4().to_string(),ids.installation_id,resource.path,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO installation_locations(id,installation_id,path_display,role,last_verified_at_ms) VALUES (?1,?2,?3,'EXECUTABLE',?4)",
            params![Uuid::new_v4().to_string(),ids.installation_id,std::path::Path::new(&resource.path).join(&request.executable_relative_path).to_string_lossy(),now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO cask_installations(id,cask_id,installation_id,relationship,removal_policy,created_at_ms) VALUES (?1,?2,?3,'INSTALLED_FOR_CASK','PLAN_REQUIRED',?4)",
            params![ids.cask_installation_id,cask.id,ids.installation_id,now]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE intake_operations SET state='REGISTERED',updated_at_ms=?2 WHERE id=?1 AND state='PLANNED'",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute(
            "UPDATE portable_install_intents SET state='REGISTERED' WHERE operation_id=?1",
            [op_id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("UPDATE operation_steps SET state='SUCCEEDED',finished_at_ms=?2 WHERE operation_id=?1 AND sequence=1",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE operation_steps SET state='SUCCEEDED',attempt_count=1,started_at_ms=?2,finished_at_ms=?2 WHERE operation_id=?1 AND sequence=2",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![cask.id,now]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    fn finish_portable(&self, op_id: &str, now: i64) -> Result<(), String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        let changed=tx.execute("UPDATE intake_operations SET state='COMPLETE',updated_at_ms=?2 WHERE id=?1 AND state='REGISTERED'",params![op_id,now]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Portable resource was not registered".into());
        }
        tx.execute("UPDATE portable_install_intents SET state='COMPLETE' WHERE operation_id=?1 AND state='REGISTERED'",[op_id]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE operations SET state='SUCCEEDED',current_step_code='COMPLETE',updated_at_ms=?2,finished_at_ms=?2,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![op_id,now]).map_err(|e|e.to_string())?;
        tx.execute("DELETE FROM operation_locks WHERE operation_id=?1", [op_id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    fn fail_portable(&self, op_id: &str, code: &str) -> Result<(), String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        tx.execute("UPDATE portable_install_intents SET state='RECOVERY_REQUIRED' WHERE operation_id=?1 AND state='PLANNED'",[op_id]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE intake_operations SET state='RECOVERY_REQUIRED' WHERE id=?1 AND state='PLANNED'",[op_id]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE operations SET state='RECOVERY_REQUIRED',current_step_code='RECONCILE',error_code=?2,revision=revision+1 WHERE id=?1 AND state='RUNNING'",params![op_id,code]).map_err(|e|e.to_string())?;
        tx.execute("UPDATE operation_steps SET state='RECOVERY_REQUIRED' WHERE operation_id=?1 AND state='RUNNING'",[op_id]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    fn list_portable(&self, cask_id: &str) -> Result<Vec<PortableInstallationView>, String> {
        let connection = self.connection()?;
        let mut stmt = connection
            .prepare(&format!(
                "{SELECT_PORTABLE} WHERE ci.cask_id=?1 ORDER BY i.installed_at_ms,i.id"
            ))
            .map_err(|e| e.to_string())?;
        stmt.query_map([cask_id], row)
            .map_err(|e| e.to_string())?
            .map(|r| r.map(|v| v.view))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    fn get_portable(&self, id: &str) -> Result<Option<PortableRecord>, String> {
        self.connection()?
            .query_row(&format!("{SELECT_PORTABLE} WHERE i.id=?1"), [id], row)
            .optional()
            .map_err(|e| e.to_string())
    }
    fn record_launch(&self, id: &str, success: bool, now: i64) -> Result<(), String> {
        self.connection()?.execute("INSERT INTO portable_launches(id,installation_id,state,at_ms) VALUES (?1,?2,?3,?4)",
            params![Uuid::new_v4().to_string(),id,if success {"STARTED"} else {"FAILED"},now]).map_err(|e|e.to_string())?;
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::LocalStorage;
    use casklet_application::CaskService;
    use casklet_contracts::{
        CreateCask, CreateRemovalPlan, InspectPortableZip, InstallPortableZip,
    };
    use std::{fs, io::Write};
    use zip::{ZipWriter, write::SimpleFileOptions};

    #[test]
    fn portable_install_records_distinct_identities_and_uses_removal_plan() {
        let root = std::env::temp_dir().join(format!("casklet-portable-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let archive = root.join("app.zip");
        let mut writer = ZipWriter::new(fs::File::create(&archive).unwrap());
        writer
            .start_file("app.exe", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"MZ-portable-test").unwrap();
        writer.finish().unwrap();
        let service = CaskService {
            repository: SqliteCasks::open(root.join("state.db")).unwrap(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Portable".into(),
                },
                "test",
            )
            .unwrap();
        let preview = service
            .inspect_portable_zip(
                &InspectPortableZip {
                    version: CONTRACT_VERSION,
                    archive_path: archive.to_string_lossy().into_owned(),
                },
                "test",
            )
            .unwrap();
        let item = service
            .install_portable_zip(
                &InstallPortableZip {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    archive_path: archive.to_string_lossy().into_owned(),
                    archive_sha256: preview.archive_sha256,
                    application_name: "App".into(),
                    application_version: Some("1.0".into()),
                    executable_relative_path: "app.exe".into(),
                },
                "test",
                &|_, _| {},
                &|_| {},
            )
            .unwrap();
        assert_ne!(item.application_id, item.installation_id);
        assert_ne!(item.installation_id, item.resource_id);
        assert_eq!(item.provider, "PORTABLE_ZIP");
        assert_eq!(item.relationship, "INSTALLED_FOR_CASK");
        let plan = service
            .create_removal_plan(
                &CreateRemovalPlan {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id,
                    resource_ids: vec![item.resource_id],
                    category_key: None,
                    permanent: false,
                },
                "test",
            )
            .unwrap();
        assert_eq!(plan.items[0].action, "TRASH");
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }
}
