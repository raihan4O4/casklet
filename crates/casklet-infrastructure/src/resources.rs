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
        self.connection()?.execute("INSERT INTO categories(key,cask_id,display_name,is_builtin,sort_order) VALUES (?1,?2,?3,0,100)",
            params![key,cask_id,name]).map_err(|e| e.to_string())?;
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
        self.connection()?.execute("INSERT INTO intake_operations(id,cask_id,resource_id,mode,source_path,staging_path,final_path,state,created_at_ms,updated_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,'PLANNED',?8,?8)",
            params![op_id,resource.cask_id,resource.id,mode.code(),source,staging,final_path,resource.created_at_ms])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn commit_resource(&self, resource: &Resource, op_id: Option<&str>) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
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
        transaction.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms) VALUES (?1,?2,?3,?4,?5,1,?6)",
            params![Uuid::new_v4().to_string(),resource.id,resource.path,resource.path.to_lowercase(),resource.containment(),resource.created_at_ms])
            .map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![Uuid::new_v4().to_string(),resource.cask_id,resource.id,resource.category_key,resource.relationship(),
                if op_id.is_some() {"DELETE_WHEN_UNREFERENCED"} else {"DETACH_ONLY"},resource.created_at_ms])
            .map_err(|e| e.to_string())?;
        if let Some(op_id) = op_id {
            transaction.execute("UPDATE intake_operations SET state='REGISTERED',updated_at_ms=?2 WHERE id=?1 AND state='PLANNED'",
                params![op_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }
    fn complete_intake(&self, op_id: &str) -> Result<(), String> {
        self.connection()?
            .execute(
                "UPDATE intake_operations SET state='COMPLETE' WHERE id=?1 AND state='REGISTERED'",
                [op_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
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
        self.connection()?.execute("UPDATE resource_associations SET category_key=?2,revision=revision+1 WHERE resource_id=?1 AND removed_at_ms IS NULL", params![id,key])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
