//! Immutable SQLite removal previews and conservative managed-path inspection.
use super::{LocalStorage, SqliteCasks, resource_storage::check_components};
use casklet_application::{
    removal::{PlannedItem, RemovalCandidate, RemovalInspection, RemovalRepository},
    resource::ResourceStorage,
};
use casklet_contracts::{CONTRACT_VERSION, RemovalPlanItemView, RemovalPlanView};
use casklet_domain::resource::ResourceKind;
use rusqlite::{OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

type PlanHeader = (
    String,
    String,
    Option<String>,
    String,
    String,
    i64,
    i64,
    i64,
    bool,
    bool,
);

pub(super) fn paths_overlap(left: &str, right: &str) -> bool {
    let left = std::path::PathBuf::from(left.to_lowercase());
    let right = std::path::PathBuf::from(right.to_lowercase());
    left.starts_with(&right) || right.starts_with(&left)
}

pub(super) fn overlap_count(
    connection: &rusqlite::Connection,
    resource_id: &str,
    cask_id: &str,
    path: &str,
) -> Result<u64, String> {
    let mut statement = connection.prepare("SELECT a.resource_id,a.cask_id,l.path_display FROM resource_associations a JOIN resource_locations l ON l.resource_id=a.resource_id AND l.is_primary=1 AND l.retired_at_ms IS NULL WHERE a.removed_at_ms IS NULL").map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut count = 0;
    for row in rows {
        let (other_resource, other_cask, other_path) = row.map_err(|e| e.to_string())?;
        if !(other_resource == resource_id && other_cask == cask_id)
            && (other_resource == resource_id || paths_overlap(path, &other_path))
        {
            count += 1;
        }
    }
    Ok(count)
}

fn equivalent_items(left: &[PlannedItem], right: &[PlannedItem]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.view.resource_id == b.view.resource_id
                && a.view.name == b.view.name
                && a.view.action == b.view.action
                && a.view.reason_code == b.view.reason_code
                && a.view.expected_size_bytes == b.view.expected_size_bytes
                && a.path == b.path
                && a.kind == b.kind
                && a.identity == b.identity
                && a.resource_revision == b.resource_revision
                && a.association_revision == b.association_revision
                && a.other_associations == b.other_associations
        })
}

fn quarantine_path(
    cask_root: &str,
    target: &str,
    external: bool,
    run_id: &str,
    item_id: &str,
) -> Result<std::path::PathBuf, String> {
    if external {
        Ok(Path::new(target)
            .parent()
            .ok_or("Resource has no parent")?
            .join(format!(".casklet-removal-{run_id}-{item_id}")))
    } else {
        Ok(Path::new(cask_root)
            .join(".casklet")
            .join("removal-staging")
            .join(run_id)
            .join(item_id))
    }
}

impl LocalStorage {
    fn verify_quarantined(
        &self,
        quarantine: &Path,
        kind: &str,
        expected_identity: &str,
    ) -> Result<(), String> {
        check_components(quarantine)?;
        let observed = self.inspect(&quarantine.to_string_lossy(), &|_, _| {})?;
        if observed.kind.code() != kind
            || observed.recordable_identity.as_deref() != Some(expected_identity)
        {
            return Err("Quarantined resource changed; item retained for recovery".into());
        }
        Ok(())
    }

    fn stage_verified_removal(
        &self,
        cask_root: &str,
        candidate: &RemovalCandidate,
        expected_identity: &str,
        run_id: &str,
        item_id: &str,
    ) -> Result<std::path::PathBuf, String> {
        if uuid::Uuid::parse_str(run_id).is_err() || uuid::Uuid::parse_str(item_id).is_err() {
            return Err("Invalid removal run or item ID".into());
        }
        if self.verify_managed(cask_root, candidate).as_deref() != Some(expected_identity) {
            return Err("Managed resource identity changed before quarantine".into());
        }
        let quarantine = quarantine_path(
            cask_root,
            &candidate.path,
            candidate.containment == "EXTERNAL",
            run_id,
            item_id,
        )?;
        let parent = quarantine.parent().ok_or("Invalid quarantine path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        check_components(parent)?;
        if fs::symlink_metadata(&quarantine).is_ok() {
            return Err("Quarantine destination is occupied".into());
        }
        check_components(Path::new(&candidate.path))?;
        fs::rename(&candidate.path, &quarantine).map_err(|e| e.to_string())?;
        self.verify_quarantined(&quarantine, &candidate.kind, expected_identity)?;
        Ok(quarantine)
    }
}

fn hash_field(hash: &mut Sha256, value: &str) {
    hash.update(
        u64::try_from(value.len())
            .expect("field length fits u64")
            .to_le_bytes(),
    );
    hash.update(value.as_bytes());
}

fn plan_hash(plan: &RemovalPlanView, items: &[PlannedItem], cask_revision: i64) -> String {
    let mut hash = Sha256::new();
    for value in [
        &plan.id,
        &plan.cask_id,
        &plan.scope,
        &plan.category_key.clone().unwrap_or_default(),
        &plan.created_at_ms.to_string(),
        &plan.expires_at_ms.to_string(),
        &cask_revision.to_string(),
    ] {
        hash_field(&mut hash, value);
    }
    if plan.cleanup_only {
        hash_field(&mut hash, "RESOURCE_CLEANUP_ONLY_V1");
    }
    if plan.permanent {
        hash_field(&mut hash, "PERMANENT_FILE_DISPOSAL_V1");
    }
    for item in items {
        for value in [
            &item.view.id,
            &item.view.resource_id,
            &item.view.name,
            &item.view.action,
            &item.view.reason_code,
            &item
                .view
                .expected_size_bytes
                .map_or_else(String::new, |v| v.to_string()),
            &item.path,
            &item.kind,
            &item.identity.clone().unwrap_or_default(),
            &item.resource_revision.to_string(),
            &item.association_revision.to_string(),
            &item.other_associations.to_string(),
        ] {
            hash_field(&mut hash, value);
        }
    }
    format!("{:x}", hash.finalize())
}

fn load_plan_items(
    connection: &rusqlite::Connection,
    id: &str,
) -> Result<Vec<PlannedItem>, String> {
    let mut stmt = connection.prepare("SELECT id,resource_id,name,action,reason_code,expected_size_bytes,expected_path,expected_kind,expected_identity,resource_revision,association_revision,other_associations,state,result_code FROM removal_plan_items WHERE plan_id=?1 ORDER BY sequence").map_err(|e|e.to_string())?;
    stmt.query_map([id], |row| {
        Ok(PlannedItem {
            view: RemovalPlanItemView {
                id: row.get(0)?,
                resource_id: row.get(1)?,
                name: row.get(2)?,
                path: row.get(6)?,
                action: row.get(3)?,
                reason_code: row.get(4)?,
                expected_size_bytes: row
                    .get::<_, Option<i64>>(5)?
                    .and_then(|n| u64::try_from(n).ok()),
                state: row.get(12)?,
                result_code: row.get(13)?,
                recovery_path: None,
            },
            path: row.get(6)?,
            kind: row.get(7)?,
            identity: row.get(8)?,
            resource_revision: row.get(9)?,
            association_revision: row.get(10)?,
            other_associations: u64::try_from(row.get::<_, i64>(11)?).unwrap_or(u64::MAX),
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

impl RemovalRepository for SqliteCasks {
    fn plan_items(&self, id: &str) -> Result<Vec<PlannedItem>, String> {
        load_plan_items(&self.connection()?, id)
    }
    fn approve_plan(&self, id: &str, hash: &str, now_ms: i64) -> Result<(), String> {
        // Re-reading verifies the immutable snapshot hash before changing its state.
        self.get_plan(id)?.ok_or("Removal plan not found")?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let (cask_id, scope, category, expected_revision, expires_at, stored_hash, state): (String, String, Option<String>, i64, i64, String, String) = transaction.query_row(
            "SELECT cask_id,scope,category_key,based_on_cask_revision,expires_at_ms,plan_hash,state FROM removal_plans WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        ).map_err(|e|e.to_string())?;
        if state != "DRAFT" || stored_hash != hash || expires_at <= now_ms {
            return Err("Plan is changed, expired, or already acted on".into());
        }
        let (current_revision, cask_state): (i64, String) = transaction
            .query_row(
                "SELECT revision,state FROM casks WHERE id=?1",
                [&cask_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if current_revision != expected_revision
            || !matches!(cask_state.as_str(), "ACTIVE" | "PARTIALLY_DESTROYED")
        {
            return Err("Cask changed after planning".into());
        }
        let item_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM removal_plan_items WHERE plan_id=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if item_count == 0 {
            return Err("Empty plan cannot be approved".into());
        }
        if scope != "RESOURCES" {
            let actual_count: i64 = transaction.query_row("SELECT COUNT(*) FROM resource_associations WHERE cask_id=?1 AND removed_at_ms IS NULL AND (?2 IS NULL OR category_key=?2)", params![cask_id,category], |row|row.get(0)).map_err(|e|e.to_string())?;
            if actual_count != item_count {
                return Err("Removal scope changed after planning".into());
            }
        }
        for item in load_plan_items(&transaction, id)? {
            if overlap_count(&transaction, &item.view.resource_id, &cask_id, &item.path)?
                != item.other_associations
            {
                return Err("Resource sharing changed after planning".into());
            }
        }
        let stale_count: i64 = transaction.query_row("SELECT COUNT(*) FROM removal_plan_items i LEFT JOIN resources r ON r.id=i.resource_id LEFT JOIN resource_associations a ON a.resource_id=i.resource_id AND a.cask_id=?2 AND a.removed_at_ms IS NULL LEFT JOIN resource_locations l ON l.resource_id=i.resource_id AND l.is_primary=1 AND l.retired_at_ms IS NULL WHERE i.plan_id=?1 AND (r.id IS NULL OR a.id IS NULL OR l.id IS NULL OR r.revision<>i.resource_revision OR a.revision<>i.association_revision OR r.kind<>i.expected_kind OR l.path_display<>i.expected_path OR l.identity_key IS NOT i.expected_identity)", params![id,cask_id], |row|row.get(0)).map_err(|e|e.to_string())?;
        if stale_count != 0 {
            return Err("Resource or association changed after planning".into());
        }
        transaction.execute("UPDATE removal_plans SET state='APPROVED',approved_at_ms=?2,revision=revision+1 WHERE id=?1 AND state='DRAFT'",params![id,now_ms]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn begin_execution(
        &self,
        id: &str,
        hash: &str,
        run_id: &str,
        now_ms: i64,
    ) -> Result<(), String> {
        self.get_plan(id)?.ok_or("Removal plan not found")?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let (cask_id, scope, category, revision, expires, stored_hash, state): (String,String,Option<String>,i64,i64,String,String) = transaction.query_row(
            "SELECT cask_id,scope,category_key,based_on_cask_revision,expires_at_ms,plan_hash,state FROM removal_plans WHERE id=?1",[id],
            |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)),
        ).map_err(|e|e.to_string())?;
        if state != "APPROVED" || stored_hash != hash || expires <= now_ms {
            return Err("Removal approval is stale".into());
        }
        let (current_revision, cask_state): (i64, String) = transaction
            .query_row(
                "SELECT revision,state FROM casks WHERE id=?1",
                [&cask_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        if current_revision != revision
            || !matches!(cask_state.as_str(), "ACTIVE" | "PARTIALLY_DESTROYED")
        {
            return Err("Cask changed after approval".into());
        }
        let active_intake: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM operation_locks WHERE lock_key=?1",
                [format!("cask:{cask_id}:mutation")],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if active_intake != 0 {
            return Err("Cask has an active or unrecovered intake".into());
        }
        let active_installer: i64 = transaction
            .query_row(
                "SELECT (SELECT COUNT(*) FROM installer_locks WHERE cask_id=?1)+(SELECT COUNT(*) FROM msi_cleanup_locks WHERE cask_id=?1)",
                [&cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if active_installer != 0 {
            return Err("Cask has an active or uncertain installer launch".into());
        }
        let cleanup_only: bool = transaction
            .query_row(
                "SELECT cleanup_only FROM removal_plans WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let item_count: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM removal_plan_items WHERE plan_id=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if item_count == 0 {
            return Err("Empty removal plan".into());
        }
        if scope != "RESOURCES" {
            let current_count: i64 = transaction.query_row("SELECT COUNT(*) FROM resource_associations WHERE cask_id=?1 AND removed_at_ms IS NULL AND (?2 IS NULL OR category_key=?2)",params![cask_id,category],|row|row.get(0)).map_err(|e|e.to_string())?;
            if current_count != item_count {
                return Err("Removal scope changed after approval".into());
            }
        }
        for item in load_plan_items(&transaction, id)? {
            super::resources::reject_pending_intake_overlap(&transaction, &item.path)?;
        }
        transaction.execute("INSERT INTO removal_runs(id,plan_id,cask_id,state,owner_pid,started_at_ms,updated_at_ms) VALUES (?1,?2,?3,'RUNNING',?4,?5,?5)",params![run_id,id,cask_id,std::process::id().to_string(),now_ms]).map_err(|e|e.to_string())?;
        transaction
            .execute(
                "INSERT INTO removal_locks(cask_id,run_id,acquired_at_ms) VALUES (?1,?2,?3)",
                params![cask_id, run_id, now_ms],
            )
            .map_err(|e| format!("Cask has an active or unrecovered removal: {e}"))?;
        transaction.execute("UPDATE removal_plans SET state='EXECUTING',revision=revision+1 WHERE id=?1 AND state='APPROVED'",[id]).map_err(|e|e.to_string())?;
        if scope == "CASK" && !cleanup_only {
            transaction
                .execute(
                    "UPDATE casks SET state='DESTROYING',updated_at_ms=?2 WHERE id=?1",
                    params![cask_id, now_ms],
                )
                .map_err(|e| e.to_string())?;
        }
        transaction
            .execute(
                "INSERT INTO diagnostic_events(event_code,at_ms) VALUES ('CLEANUP_STARTED',?1)",
                [now_ms],
            )
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn start_item(&self, run_id: &str, item: &PlannedItem, now_ms: i64) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let cask_id: String = transaction
            .query_row(
                "SELECT cask_id FROM removal_runs WHERE id=?1",
                [run_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let overlaps = i64::try_from(overlap_count(
            &transaction,
            &item.view.resource_id,
            &cask_id,
            &item.path,
        )?)
        .map_err(|e| e.to_string())?;
        let current: i64 = transaction.query_row("SELECT COUNT(*) FROM removal_plan_items i JOIN removal_runs run ON run.plan_id=i.plan_id AND run.id=?1 AND run.state='RUNNING' JOIN removal_locks lock ON lock.run_id=run.id AND lock.cask_id=run.cask_id JOIN resources r ON r.id=i.resource_id JOIN resource_associations a ON a.resource_id=i.resource_id AND a.cask_id=run.cask_id AND a.removed_at_ms IS NULL JOIN resource_locations l ON l.resource_id=i.resource_id AND l.is_primary=1 AND l.retired_at_ms IS NULL JOIN removal_plans p ON p.id=i.plan_id WHERE i.id=?2 AND i.state='PENDING' AND i.started_at_ms IS NULL AND r.revision=i.resource_revision AND a.revision=i.association_revision AND r.kind=i.expected_kind AND l.path_display=i.expected_path AND l.identity_key IS i.expected_identity AND (?3=i.other_associations OR (i.action IN ('DETACH','PRESERVE') AND ?3<i.other_associations)) AND (i.action NOT IN ('TRASH','DELETE_PERMANENTLY') OR (r.management='MANAGED' AND ((l.containment='INTERNAL' AND r.provenance='ADDED_BY_CASKLET') OR (l.containment='EXTERNAL' AND r.provenance='USER_ADOPTED' AND EXISTS(SELECT 1 FROM resource_ownership_grants g WHERE g.resource_id=r.id AND g.cask_id=run.cask_id AND g.exact_path=l.path_display AND g.identity_key=l.identity_key))) AND a.relationship='OWNED' AND a.removal_policy='DELETE_WHEN_UNREFERENCED' AND r.availability='PRESENT' AND r.unknown_entries=0 AND i.other_associations=0 AND i.expected_identity IS NOT NULL)) AND (i.action<>'DELETE_PERMANENTLY' OR (p.permanent=1 AND r.kind='FILE' AND l.containment='INTERNAL'))",params![run_id,item.view.id,overlaps],|row|row.get(0)).map_err(|e|e.to_string())?;
        if current != 1 {
            return Err("Removal item changed before execution; recovery required".into());
        }
        let changed = transaction.execute("UPDATE removal_plan_items SET run_id=?2,started_at_ms=?3 WHERE id=?1 AND state='PENDING' AND run_id IS NULL",params![item.view.id,run_id,now_ms]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Removal item already started".into());
        }
        transaction.commit().map_err(|e| e.to_string())
    }

    fn finish_item(
        &self,
        run_id: &str,
        item: &PlannedItem,
        state: &str,
        result_code: &str,
        now_ms: i64,
    ) -> Result<(), String> {
        if !matches!(state, "SUCCEEDED" | "PRESERVED" | "UNKNOWN") {
            return Err("Invalid removal item outcome".into());
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction.execute("UPDATE removal_plan_items SET state=?3,result_code=?4,finished_at_ms=?5 WHERE id=?1 AND run_id=?2 AND state='PENDING' AND started_at_ms IS NOT NULL",params![item.view.id,run_id,state,result_code,now_ms]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Removal item boundary changed".into());
        }
        if state == "SUCCEEDED" {
            if !matches!(
                item.view.action.as_str(),
                "TRASH" | "DETACH" | "DELETE_PERMANENTLY"
            ) {
                return Err("Invalid successful action".into());
            }
            let cask_id: String = transaction
                .query_row(
                    "SELECT cask_id FROM removal_runs WHERE id=?1",
                    [run_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let associations = transaction.execute("UPDATE resource_associations SET removed_at_ms=?3,revision=revision+1 WHERE cask_id=?1 AND resource_id=?2 AND removed_at_ms IS NULL",params![cask_id,item.view.resource_id,now_ms]).map_err(|e|e.to_string())?;
            if associations != 1 {
                return Err("Resource association changed during cleanup".into());
            }
            if matches!(item.view.action.as_str(), "TRASH" | "DELETE_PERMANENTLY") {
                transaction.execute("UPDATE resources SET availability='MISSING',revision=revision+1 WHERE id=?1",[&item.view.resource_id]).map_err(|e|e.to_string())?;
                transaction.execute("UPDATE resource_locations SET retired_at_ms=?2 WHERE resource_id=?1 AND is_primary=1 AND retired_at_ms IS NULL",params![item.view.resource_id,now_ms]).map_err(|e|e.to_string())?;
                transaction.execute("UPDATE installations SET state='REMOVED',revision=revision+1 WHERE resource_id=?1 AND provider='PORTABLE_ZIP' AND state='INSTALLED'",[&item.view.resource_id]).map_err(|e|e.to_string())?;
            }
            transaction.execute("UPDATE cask_installations SET removed_at_ms=?3,revision=revision+1 WHERE cask_id=?1 AND installation_id IN (SELECT id FROM installations WHERE resource_id=?2) AND removed_at_ms IS NULL",params![cask_id,item.view.resource_id,now_ms]).map_err(|e|e.to_string())?;
            transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![cask_id,now_ms]).map_err(|e|e.to_string())?;
        }
        transaction
            .execute(
                "UPDATE removal_runs SET updated_at_ms=?2 WHERE id=?1",
                params![run_id, now_ms],
            )
            .map_err(|e| e.to_string())?;
        let diagnostic_code = match state {
            "SUCCEEDED" => "CLEANUP_ITEM_SUCCEEDED",
            "PRESERVED" => "CLEANUP_ITEM_PRESERVED",
            _ => "CLEANUP_ITEM_UNCERTAIN",
        };
        transaction
            .execute(
                "INSERT INTO diagnostic_events(event_code,at_ms) VALUES (?1,?2)",
                params![diagnostic_code, now_ms],
            )
            .map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn finish_execution(&self, run_id: &str, now_ms: i64) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let (plan_id,cask_id,scope): (String,String,String) = transaction.query_row("SELECT run.plan_id,run.cask_id,p.scope FROM removal_runs run JOIN removal_plans p ON p.id=run.plan_id WHERE run.id=?1 AND run.state='RUNNING'",[run_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e|e.to_string())?;
        let cleanup_only: bool = transaction
            .query_row(
                "SELECT cleanup_only FROM removal_plans WHERE id=?1",
                [&plan_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let (pending,uncertain): (i64,i64) = transaction.query_row("SELECT SUM(CASE WHEN state='PENDING' THEN 1 ELSE 0 END),SUM(CASE WHEN state IN ('UNKNOWN','FAILED') THEN 1 ELSE 0 END) FROM removal_plan_items WHERE plan_id=?1",[&plan_id],|row|Ok((row.get(0)?,row.get(1)?))).map_err(|e|e.to_string())?;
        if pending != 0 || uncertain != 0 {
            transaction.execute("UPDATE removal_runs SET state='RECOVERY_REQUIRED',updated_at_ms=?2,error_code='INCOMPLETE' WHERE id=?1",params![run_id,now_ms]).map_err(|e|e.to_string())?;
            transaction
                .execute(
                    "UPDATE removal_plans SET state='PARTIAL',revision=revision+1 WHERE id=?1",
                    [&plan_id],
                )
                .map_err(|e| e.to_string())?;
            if scope == "CASK" && !cleanup_only {
                transaction
                    .execute(
                        "UPDATE casks SET state='RECOVERY_REQUIRED',updated_at_ms=?2 WHERE id=?1",
                        params![cask_id, now_ms],
                    )
                    .map_err(|e| e.to_string())?;
            }
        } else {
            transaction
                .execute(
                    "UPDATE removal_runs SET state='SUCCEEDED',updated_at_ms=?2 WHERE id=?1",
                    params![run_id, now_ms],
                )
                .map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "UPDATE removal_plans SET state='COMPLETE',revision=revision+1 WHERE id=?1",
                    [&plan_id],
                )
                .map_err(|e| e.to_string())?;
            transaction
                .execute("DELETE FROM removal_locks WHERE run_id=?1", [run_id])
                .map_err(|e| e.to_string())?;
            if scope == "CASK" && !cleanup_only {
                let remaining: i64 = transaction.query_row("SELECT COUNT(*) FROM resource_associations WHERE cask_id=?1 AND removed_at_ms IS NULL",[&cask_id],|row|row.get(0)).map_err(|e|e.to_string())?;
                let state = if remaining == 0 {
                    "DESTROYED"
                } else {
                    "PARTIALLY_DESTROYED"
                };
                transaction.execute("UPDATE casks SET state=?2,updated_at_ms=?3,revision=revision+1 WHERE id=?1",params![cask_id,state,now_ms]).map_err(|e|e.to_string())?;
            }
        }
        transaction.commit().map_err(|e| e.to_string())
    }

    fn close_uncertain_preserved(&self, run_id: &str, now_ms: i64) -> Result<String, String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let (plan_id,cask_id,scope): (String,String,String) = transaction.query_row("SELECT run.plan_id,run.cask_id,p.scope FROM removal_runs run JOIN removal_plans p ON p.id=run.plan_id WHERE run.id=?1 AND run.state='RECOVERY_REQUIRED'",[run_id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e|e.to_string())?;
        let cleanup_only: bool = transaction
            .query_row(
                "SELECT cleanup_only FROM removal_plans WHERE id=?1",
                [&plan_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        transaction.execute("UPDATE removal_plan_items SET state='UNKNOWN',result_code='CLOSED_UNRESOLVED',finished_at_ms=?2 WHERE plan_id=?1 AND state IN ('UNKNOWN','PENDING','FAILED')",params![plan_id,now_ms]).map_err(|e|e.to_string())?;
        transaction
            .execute(
                "UPDATE removal_runs SET state='CLOSED_PRESERVED',updated_at_ms=?2 WHERE id=?1",
                params![run_id, now_ms],
            )
            .map_err(|e| e.to_string())?;
        transaction
            .execute(
                "UPDATE removal_plans SET state='COMPLETE',revision=revision+1 WHERE id=?1",
                [&plan_id],
            )
            .map_err(|e| e.to_string())?;
        transaction
            .execute("DELETE FROM removal_locks WHERE run_id=?1", [run_id])
            .map_err(|e| e.to_string())?;
        if scope == "CASK" && !cleanup_only {
            transaction.execute("UPDATE casks SET state='PARTIALLY_DESTROYED',updated_at_ms=?2,revision=revision+1 WHERE id=?1",params![cask_id,now_ms]).map_err(|e|e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(plan_id)
    }

    fn mark_execution_uncertain(&self, run_id: &str, now_ms: i64) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        transaction.execute("UPDATE removal_plan_items SET state='UNKNOWN',result_code='INTERNAL_ERROR',finished_at_ms=?2 WHERE run_id=?1 AND state='PENDING' AND started_at_ms IS NOT NULL",params![run_id,now_ms]).map_err(|e|e.to_string())?;
        transaction.execute("UPDATE removal_runs SET state='RECOVERY_REQUIRED',error_code='INTERNAL_ERROR',updated_at_ms=?2 WHERE id=?1 AND state='RUNNING'",params![run_id,now_ms]).map_err(|e|e.to_string())?;
        transaction.execute("UPDATE removal_plans SET state='PARTIAL',revision=revision+1 WHERE id=(SELECT plan_id FROM removal_runs WHERE id=?1) AND state='EXECUTING'",[run_id]).map_err(|e|e.to_string())?;
        transaction.execute("UPDATE casks SET state='RECOVERY_REQUIRED' WHERE id=(SELECT cask_id FROM removal_runs WHERE id=?1) AND state='DESTROYING'",[run_id]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn candidates(&self, cask_id: &str) -> Result<Vec<RemovalCandidate>, String> {
        let connection = self.connection()?;
        let mut stmt = connection.prepare("SELECT r.id,r.display_name,l.path_display,r.kind,r.management,l.containment,r.provenance,a.relationship,a.category_key,a.removal_policy,r.availability,r.size_bytes,r.unknown_entries,r.revision,a.revision,(SELECT COUNT(*) FROM resource_associations x WHERE x.resource_id=r.id AND x.removed_at_ms IS NULL AND x.cask_id<>a.cask_id),l.identity_key,EXISTS(SELECT 1 FROM resource_ownership_grants g WHERE g.resource_id=r.id AND g.cask_id=a.cask_id AND g.exact_path=l.path_display AND g.identity_key=l.identity_key) FROM resource_associations a JOIN resources r ON r.id=a.resource_id JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL WHERE a.cask_id=?1 AND a.removed_at_ms IS NULL ORDER BY a.added_at_ms,r.id")
            .map_err(|e| e.to_string())?;
        let mut candidates: Vec<RemovalCandidate> = stmt
            .query_map([cask_id], |row| {
                Ok(RemovalCandidate {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    kind: row.get(3)?,
                    management: row.get(4)?,
                    containment: row.get(5)?,
                    provenance: row.get(6)?,
                    relationship: row.get(7)?,
                    category_key: row.get(8)?,
                    removal_policy: row.get(9)?,
                    availability: row.get(10)?,
                    size_bytes: row
                        .get::<_, Option<i64>>(11)?
                        .and_then(|n| u64::try_from(n).ok()),
                    unknown_entries: u64::try_from(row.get::<_, i64>(12)?).unwrap_or(u64::MAX),
                    resource_revision: row.get(13)?,
                    association_revision: row.get(14)?,
                    other_associations: u64::try_from(row.get::<_, i64>(15)?).unwrap_or(u64::MAX),
                    recorded_identity: row.get(16)?,
                    explicitly_adopted: row.get(17)?,
                })
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for candidate in &mut candidates {
            candidate.other_associations =
                overlap_count(&connection, &candidate.id, cask_id, &candidate.path)?;
        }
        Ok(candidates)
    }

    fn save_plan(
        &self,
        plan: &RemovalPlanView,
        items: &[PlannedItem],
        cask_revision: i64,
    ) -> Result<String, String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let current_revision: i64 = transaction
            .query_row(
                "SELECT revision FROM casks WHERE id=?1",
                [&plan.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if current_revision != cask_revision {
            return Err("Cask changed during planning".into());
        }
        if plan.scope == "CASK" || plan.scope == "CATEGORY" {
            let count: i64 = transaction.query_row(
                "SELECT COUNT(*) FROM resource_associations WHERE cask_id=?1 AND removed_at_ms IS NULL AND (?2 IS NULL OR category_key=?2)",
                params![plan.cask_id,plan.category_key],
                |row|row.get(0),
            ).map_err(|e|e.to_string())?;
            if count != i64::try_from(items.len()).map_err(|e| e.to_string())? {
                return Err("Removal scope changed during planning".into());
            }
        }
        let reusable = {
            let mut statement = transaction.prepare("SELECT id FROM removal_plans WHERE cask_id=?1 AND scope=?2 AND category_key IS ?3 AND permanent=?4 AND cleanup_only=?5 AND based_on_cask_revision=?6 AND state IN ('DRAFT','APPROVED') AND expires_at_ms>?7 ORDER BY created_at_ms DESC").map_err(|e| e.to_string())?;
            statement
                .query_map(
                    params![
                        plan.cask_id,
                        plan.scope,
                        plan.category_key,
                        plan.permanent,
                        plan.cleanup_only,
                        cask_revision,
                        plan.created_at_ms
                    ],
                    |row| row.get::<_, String>(0),
                )
                .map_err(|e| e.to_string())?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?
        };
        for id in reusable {
            if equivalent_items(&load_plan_items(&transaction, &id)?, items) {
                transaction.commit().map_err(|e| e.to_string())?;
                return Ok(id);
            }
        }
        let hash = plan_hash(plan, items, cask_revision);
        transaction.execute("INSERT INTO removal_plans(id,cask_id,scope,category_key,state,based_on_cask_revision,plan_hash,created_at_ms,expires_at_ms,permanent,cleanup_only) VALUES (?1,?2,?3,?4,'DRAFT',?5,?6,?7,?8,?9,?10)",
            params![plan.id, plan.cask_id, plan.scope,plan.category_key, cask_revision, hash, plan.created_at_ms, plan.expires_at_ms,plan.permanent,plan.cleanup_only]).map_err(|e|e.to_string())?;
        for (sequence, item) in items.iter().enumerate() {
            let actual: (i64,i64,i64) = transaction.query_row("SELECT r.revision,a.revision,(SELECT COUNT(*) FROM resource_associations x WHERE x.resource_id=r.id AND x.removed_at_ms IS NULL AND x.cask_id<>a.cask_id) FROM resources r JOIN resource_associations a ON a.resource_id=r.id AND a.cask_id=?2 AND a.removed_at_ms IS NULL WHERE r.id=?1", params![item.view.resource_id,plan.cask_id], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e|e.to_string())?;
            let actual = (
                actual.0,
                actual.1,
                i64::try_from(overlap_count(
                    &transaction,
                    &item.view.resource_id,
                    &plan.cask_id,
                    &item.path,
                )?)
                .map_err(|e| e.to_string())?,
            );
            if actual
                != (
                    item.resource_revision,
                    item.association_revision,
                    i64::try_from(item.other_associations).map_err(|e| e.to_string())?,
                )
            {
                return Err("Resource changed during planning".into());
            }
            transaction.execute("INSERT INTO removal_plan_items(id,plan_id,sequence,resource_id,name,action,reason_code,expected_size_bytes,expected_path,expected_kind,expected_identity,resource_revision,association_revision,other_associations) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![item.view.id,plan.id,i64::try_from(sequence).map_err(|e|e.to_string())?,item.view.resource_id,item.view.name,item.view.action,item.view.reason_code,item.view.expected_size_bytes.map(i64::try_from).transpose().map_err(|e|e.to_string())?,item.path,item.kind,item.identity,item.resource_revision,item.association_revision,i64::try_from(item.other_associations).map_err(|e|e.to_string())?]).map_err(|e|e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(plan.id.clone())
    }

    fn list_plans(&self, cask_id: Option<&str>) -> Result<Vec<RemovalPlanView>, String> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT p.id FROM removal_plans p WHERE (?1 IS NULL OR p.cask_id=?1) AND NOT EXISTS(SELECT 1 FROM cask_removals x WHERE x.cask_id=p.cask_id) ORDER BY CASE WHEN p.state IN ('PARTIAL','EXECUTING') THEN 0 ELSE 1 END,p.created_at_ms DESC,p.id DESC LIMIT 50").map_err(|e| e.to_string())?;
        let ids = statement
            .query_map([cask_id], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        ids.into_iter()
            .map(|id| {
                self.get_plan(&id)?
                    .ok_or_else(|| "Cleanup plan disappeared".into())
            })
            .collect()
    }

    fn get_plan(&self, id: &str) -> Result<Option<RemovalPlanView>, String> {
        let connection = self.connection()?;
        let header: Option<PlanHeader> = connection.query_row("SELECT cask_id,scope,category_key,state,plan_hash,created_at_ms,expires_at_ms,based_on_cask_revision,permanent,cleanup_only FROM removal_plans WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?))).optional().map_err(|e|e.to_string())?;
        let Some((
            cask_id,
            scope,
            category_key,
            state,
            stored_hash,
            created_at_ms,
            expires_at_ms,
            cask_revision,
            permanent,
            cleanup_only,
        )) = header
        else {
            return Ok(None);
        };
        let snapshots = load_plan_items(&connection, id)?;
        let run_id: Option<String> = connection
            .query_row(
                "SELECT id FROM removal_runs WHERE plan_id=?1 ORDER BY started_at_ms DESC LIMIT 1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let mut view = RemovalPlanView {
            version: CONTRACT_VERSION,
            id: id.into(),
            cask_id,
            scope,
            category_key,
            permanent,
            cleanup_only,
            state,
            plan_hash: stored_hash,
            run_id,
            created_at_ms,
            expires_at_ms,
            items: snapshots.iter().map(|item| item.view.clone()).collect(),
        };
        if view.state != "INVALIDATED"
            && plan_hash(&view, &snapshots, cask_revision) != view.plan_hash
        {
            return Err("Removal plan integrity mismatch".into());
        }
        if let Some(run_id) = &view.run_id
            && uuid::Uuid::parse_str(run_id).is_ok()
        {
            let root: String = connection
                .query_row(
                    "SELECT managed_storage_root FROM casks WHERE id=?1",
                    [&view.cask_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            for item in &mut view.items {
                if item.state == "UNKNOWN" && uuid::Uuid::parse_str(&item.id).is_ok() {
                    let snapshot = snapshots
                        .iter()
                        .find(|snapshot| snapshot.view.id == item.id)
                        .ok_or("Missing recovery snapshot")?;
                    let external =
                        !Path::new(&snapshot.path).starts_with(Path::new(&root).join("content"));
                    let location =
                        quarantine_path(&root, &snapshot.path, external, run_id, &item.id)?;
                    if fs::symlink_metadata(&location).is_ok() {
                        item.recovery_path = Some(location.to_string_lossy().into_owned());
                    }
                }
            }
        }
        Ok(Some(view))
    }
}

impl RemovalInspection for LocalStorage {
    fn verify_managed(&self, cask_root: &str, candidate: &RemovalCandidate) -> Option<String> {
        if candidate.management != "MANAGED" {
            return None;
        }
        if candidate.containment == "EXTERNAL" {
            if !candidate.explicitly_adopted
                || candidate.provenance != "USER_ADOPTED"
                || super::resource_storage::validate_owned_path(
                    &self.root,
                    Path::new(&candidate.path),
                )
                .is_err()
            {
                return None;
            }
            let observed = self.inspect(&candidate.path, &|_, _| {}).ok()?;
            if observed.kind.code() != candidate.kind
                || observed.size_bytes != candidate.size_bytes
                || observed.unknown_entries > 0
            {
                return None;
            }
            let identity = observed.recordable_identity?;
            return (candidate.recorded_identity.as_deref() == Some(identity.as_str()))
                .then_some(identity);
        }
        if candidate.containment != "INTERNAL" {
            return None;
        }
        let kind = ResourceKind::from_code(&candidate.kind)?;
        let root = Path::new(cask_root);
        let target = Path::new(&candidate.path);
        if root.parent() != Some(self.root.as_path()) {
            return None;
        }
        let extension = Path::new(&candidate.name)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()))
            .map_or_else(String::new, |e| format!(".{e}"));
        let expected = root
            .join("content")
            .join("other")
            .join(format!("{}{extension}", candidate.id));
        if target != expected
            || !target.is_absolute()
            || check_components(target).is_err()
            || check_components(root).is_err()
        {
            return None;
        }
        let root_canonical = fs::canonicalize(root).ok()?;
        let target_canonical = fs::canonicalize(target).ok()?;
        if !target_canonical.starts_with(root_canonical.join("content").join("other")) {
            return None;
        }
        let observed = self.inspect(&candidate.path, &|_, _| {}).ok()?;
        if observed.kind != kind
            || observed.size_bytes != candidate.size_bytes
            || observed.unknown_entries > 0
        {
            return None;
        }
        let current = observed.recordable_identity?;
        if candidate.recorded_identity.as_deref() != Some(current.as_str()) {
            return None;
        }
        Some(current)
    }

    fn trash_verified(
        &self,
        cask_root: &str,
        candidate: &RemovalCandidate,
        expected_identity: &str,
        run_id: &str,
        item_id: &str,
    ) -> Result<(), String> {
        if !matches!(candidate.kind.as_str(), "FILE" | "DIRECTORY") {
            return Err("Only identified managed resources may be trashed".into());
        }
        let quarantine =
            self.stage_verified_removal(cask_root, candidate, expected_identity, run_id, item_id)?;
        self.verify_quarantined(&quarantine, &candidate.kind, expected_identity)?;
        trash::delete(&quarantine).map_err(|e| e.to_string())
    }

    fn delete_permanently_verified(
        &self,
        cask_root: &str,
        candidate: &RemovalCandidate,
        expected_identity: &str,
        run_id: &str,
        item_id: &str,
    ) -> Result<(), String> {
        if candidate.kind != "FILE" || candidate.containment != "INTERNAL" {
            return Err("Permanent deletion supports identified managed files only".into());
        }
        let quarantine =
            self.stage_verified_removal(cask_root, candidate, expected_identity, run_id, item_id)?;
        self.verify_quarantined(&quarantine, &candidate.kind, expected_identity)?;
        fs::remove_file(&quarantine).map_err(|e| e.to_string())
    }
}

impl SqliteCasks {
    /// Marks interrupted cleanup as uncertain and keeps its mutation lock for inspection.
    /// # Errors
    /// Returns a persistence error.
    pub fn detect_abandoned_removals(&self) -> Result<(), String> {
        self.detect_abandoned_removals_with(super::operations::owner_process_alive)
    }

    pub(super) fn detect_abandoned_removals_with(
        &self,
        owner_alive: impl Fn(&str) -> bool,
    ) -> Result<(), String> {
        let mut connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT r.id,r.owner_pid,l.run_id FROM removal_runs r LEFT JOIN removal_locks l ON l.run_id=r.id WHERE r.state='RUNNING'")
            .map_err(|e| e.to_string())?;
        let runs = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        drop(statement);
        for (run_id, owner, lock) in runs {
            if lock.is_some() && owner_alive(&owner) {
                continue;
            }
            let transaction = connection.transaction().map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "UPDATE removal_plan_items SET state='UNKNOWN',result_code='INTERRUPTED',finished_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE run_id=?1 AND state='PENDING' AND started_at_ms IS NOT NULL",
                    [&run_id],
                )
                .map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "UPDATE removal_runs SET state='RECOVERY_REQUIRED',error_code='INTERRUPTED',updated_at_ms=CAST(unixepoch('subsec')*1000 AS INTEGER) WHERE id=?1 AND state='RUNNING'",
                    [&run_id],
                )
                .map_err(|e| e.to_string())?;
            transaction
                .execute(
                    "UPDATE removal_plans SET state='PARTIAL',revision=revision+1 WHERE id=(SELECT plan_id FROM removal_runs WHERE id=?1) AND state='EXECUTING'",
                    [&run_id],
                )
                .map_err(|e| e.to_string())?;
            transaction.execute("UPDATE casks SET state='RECOVERY_REQUIRED' WHERE id=(SELECT cask_id FROM removal_runs WHERE id=?1) AND state='DESTROYING'",[&run_id]).map_err(|e|e.to_string())?;
            transaction.commit().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use casklet_application::removal::RemovalInspection;

    struct UncertainTrash(LocalStorage);
    impl RemovalInspection for UncertainTrash {
        fn verify_managed(&self, root: &str, candidate: &RemovalCandidate) -> Option<String> {
            self.0.verify_managed(root, candidate)
        }
        fn trash_verified(
            &self,
            root: &str,
            candidate: &RemovalCandidate,
            identity: &str,
            run: &str,
            item: &str,
        ) -> Result<(), String> {
            self.0
                .stage_verified_removal(root, candidate, identity, run, item)?;
            Err("Simulated handoff interruption; quarantined data retained".into())
        }
    }

    fn external_test_root() -> std::path::PathBuf {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(format!("external-quarantine-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn external_quarantine_is_journaled_on_the_source_volume_and_blocks_mutation() {
        use casklet_application::CaskService;
        use casklet_contracts::{
            AdoptResource, ApproveRemovalPlan, CONTRACT_VERSION, CreateCask, CreateRemovalPlan,
            ExecuteRemovalPlan, SetCaskArchived,
        };
        let root = external_test_root();
        let repository = SqliteCasks::open(root.join("db/casklet.db")).unwrap();
        let service = CaskService {
            repository: repository.clone(),
            storage: LocalStorage::new(root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "External".into(),
                },
                "test",
            )
            .unwrap();
        let source = root.join("project");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("keep.txt"), b"recover me").unwrap();
        let owned = service
            .adopt_resource(
                &AdoptResource {
                    version: CONTRACT_VERSION,
                    cask_id: cask.id.clone(),
                    source_path: source.to_string_lossy().into_owned(),
                    category_key: "other".into(),
                    confirm_ownership: true,
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
        let executor = CaskService {
            repository: repository.clone(),
            storage: UncertainTrash(LocalStorage::new(root.join("casks"))),
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
        let recovery = Path::new(result.items[0].recovery_path.as_ref().unwrap());
        assert_eq!(recovery.parent(), Path::new(&owned.path).parent());
        assert_eq!(fs::read(recovery.join("keep.txt")).unwrap(), b"recover me");
        assert!(!source.exists());
        assert!(
            service
                .set_cask_archived(
                    &SetCaskArchived {
                        version: CONTRACT_VERSION,
                        id: cask.id,
                        archived: true,
                        expected_revision: cask.revision
                    },
                    "test"
                )
                .is_err()
        );
        fs::rename(recovery, &source).unwrap();
        assert_eq!(fs::read(source.join("keep.txt")).unwrap(), b"recover me");
        drop(executor);
        drop(service);
        drop(repository);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn quarantine_replacement_with_identical_bytes_is_rejected() {
        let root =
            std::env::temp_dir().join(format!("casklet-quarantine-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let quarantine = root.join("quarantine.txt");
        fs::write(&quarantine, b"same bytes").unwrap();
        let storage = LocalStorage::new(root.clone());
        let identity = storage
            .inspect(&quarantine.to_string_lossy(), &|_, _| {})
            .unwrap()
            .recordable_identity
            .unwrap();
        let retained = root.join("retained.txt");
        fs::rename(&quarantine, &retained).unwrap();
        fs::write(&quarantine, b"same bytes").unwrap();
        assert!(
            storage
                .verify_quarantined(&quarantine, "FILE", &identity)
                .is_err()
        );
        assert!(retained.exists());
        assert!(quarantine.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
