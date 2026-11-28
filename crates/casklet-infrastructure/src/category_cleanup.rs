//! Durable whole-Cask intent; existing component journals and tombstones own outcomes.
use super::SqliteCasks;
use casklet_application::category_cleanup::{WholeCaskCleanupRepository, WholeCaskIntent};
use rusqlite::{OptionalExtension, params};

impl WholeCaskCleanupRepository for SqliteCasks {
    fn create_whole_cask_intent(
        &self,
        cask: &str,
        resources: Option<&str>,
        applications: Option<&str>,
    ) -> Result<String, String> {
        let mut connection = self.connection()?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        let active: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM casks WHERE id=?1 AND state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM cask_removals WHERE cask_id=?1))",[cask],|row|row.get(0)).map_err(|e|e.to_string())?;
        if !active {
            return Err("Cask is no longer active".into());
        }
        let existing: Option<String> = tx.query_row("SELECT id FROM whole_cask_cleanup_intents WHERE cask_id=?1 AND resource_plan_id IS ?2 AND application_plan_id IS ?3 ORDER BY created_at_ms DESC LIMIT 1",params![cask,resources,applications],|row|row.get(0)).optional().map_err(|e|e.to_string())?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let id = uuid::Uuid::new_v4().to_string();
        tx.execute("INSERT INTO whole_cask_cleanup_intents(id,cask_id,resource_plan_id,application_plan_id,created_at_ms) VALUES (?1,?2,?3,?4,CAST(unixepoch('subsec')*1000 AS INTEGER))",params![id,cask,resources,applications]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(id)
    }
    fn whole_cask_intent(
        &self,
        cask: &str,
        id: Option<&str>,
        resource_plan: Option<&str>,
    ) -> Result<Option<WholeCaskIntent>, String> {
        self.connection()?.query_row("SELECT i.id,i.cask_id,i.resource_plan_id,i.application_plan_id,r.cask_id IS NOT NULL,r.storage_warning FROM whole_cask_cleanup_intents i LEFT JOIN cask_removals r ON r.cask_id=i.cask_id WHERE i.cask_id=?1 AND (?2 IS NULL OR i.id=?2) AND (?3 IS NULL OR i.resource_plan_id=?3) ORDER BY i.created_at_ms DESC,i.id DESC LIMIT 1",params![cask,id,resource_plan],|row|Ok(WholeCaskIntent { id:row.get(0)?,cask_id:row.get(1)?,resource_plan_id:row.get(2)?,application_plan_id:row.get(3)?,removed:row.get(4)?,warning:row.get(5)? })).optional().map_err(|e|e.to_string())
    }
}
