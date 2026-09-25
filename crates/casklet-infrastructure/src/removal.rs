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
);

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
    let mut stmt = connection.prepare("SELECT id,resource_id,name,action,reason_code,expected_size_bytes,expected_path,expected_kind,expected_identity,resource_revision,association_revision,other_associations FROM removal_plan_items WHERE plan_id=?1 ORDER BY sequence").map_err(|e|e.to_string())?;
    stmt.query_map([id], |row| {
        Ok(PlannedItem {
            view: RemovalPlanItemView {
                id: row.get(0)?,
                resource_id: row.get(1)?,
                name: row.get(2)?,
                action: row.get(3)?,
                reason_code: row.get(4)?,
                expected_size_bytes: row
                    .get::<_, Option<i64>>(5)?
                    .and_then(|n| u64::try_from(n).ok()),
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
        if current_revision != expected_revision || cask_state != "ACTIVE" {
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
        let stale_count: i64 = transaction.query_row("SELECT COUNT(*) FROM removal_plan_items i LEFT JOIN resources r ON r.id=i.resource_id LEFT JOIN resource_associations a ON a.resource_id=i.resource_id AND a.cask_id=?2 AND a.removed_at_ms IS NULL LEFT JOIN resource_locations l ON l.resource_id=i.resource_id AND l.is_primary=1 AND l.retired_at_ms IS NULL WHERE i.plan_id=?1 AND (r.id IS NULL OR a.id IS NULL OR l.id IS NULL OR r.revision<>i.resource_revision OR a.revision<>i.association_revision OR r.kind<>i.expected_kind OR l.path_display<>i.expected_path OR l.identity_key IS NOT i.expected_identity OR (SELECT COUNT(*) FROM resource_associations x WHERE x.resource_id=i.resource_id AND x.cask_id<>?2 AND x.removed_at_ms IS NULL)<>i.other_associations)", params![id,cask_id], |row|row.get(0)).map_err(|e|e.to_string())?;
        if stale_count != 0 {
            return Err("Resource or association changed after planning".into());
        }
        transaction.execute("UPDATE removal_plans SET state='APPROVED',approved_at_ms=?2,revision=revision+1 WHERE id=?1 AND state='DRAFT'",params![id,now_ms]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())
    }

    fn candidates(&self, cask_id: &str) -> Result<Vec<RemovalCandidate>, String> {
        let connection = self.connection()?;
        let mut stmt = connection.prepare("SELECT r.id,r.display_name,l.path_display,r.kind,r.management,l.containment,r.provenance,a.relationship,a.category_key,a.removal_policy,r.availability,r.size_bytes,r.unknown_entries,r.revision,a.revision,(SELECT COUNT(*) FROM resource_associations x WHERE x.resource_id=r.id AND x.removed_at_ms IS NULL AND x.cask_id<>a.cask_id),l.identity_key FROM resource_associations a JOIN resources r ON r.id=a.resource_id JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL WHERE a.cask_id=?1 AND a.removed_at_ms IS NULL ORDER BY a.added_at_ms,r.id")
            .map_err(|e| e.to_string())?;
        stmt.query_map([cask_id], |row| {
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
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }

    fn save_plan(
        &self,
        plan: &RemovalPlanView,
        items: &[PlannedItem],
        cask_revision: i64,
    ) -> Result<(), String> {
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
        let hash = plan_hash(plan, items, cask_revision);
        transaction.execute("INSERT INTO removal_plans(id,cask_id,scope,category_key,state,based_on_cask_revision,plan_hash,created_at_ms,expires_at_ms) VALUES (?1,?2,?3,?4,'DRAFT',?5,?6,?7,?8)",
            params![plan.id, plan.cask_id, plan.scope,plan.category_key, cask_revision, hash, plan.created_at_ms, plan.expires_at_ms]).map_err(|e|e.to_string())?;
        for (sequence, item) in items.iter().enumerate() {
            let actual: (i64,i64,i64) = transaction.query_row("SELECT r.revision,a.revision,(SELECT COUNT(*) FROM resource_associations x WHERE x.resource_id=r.id AND x.removed_at_ms IS NULL AND x.cask_id<>a.cask_id) FROM resources r JOIN resource_associations a ON a.resource_id=r.id AND a.cask_id=?2 AND a.removed_at_ms IS NULL WHERE r.id=?1", params![item.view.resource_id,plan.cask_id], |row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(|e|e.to_string())?;
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
        transaction.commit().map_err(|e| e.to_string())
    }

    fn get_plan(&self, id: &str) -> Result<Option<RemovalPlanView>, String> {
        let connection = self.connection()?;
        let header: Option<PlanHeader> = connection.query_row("SELECT cask_id,scope,category_key,state,plan_hash,created_at_ms,expires_at_ms,based_on_cask_revision FROM removal_plans WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?))).optional().map_err(|e|e.to_string())?;
        let Some((
            cask_id,
            scope,
            category_key,
            state,
            stored_hash,
            created_at_ms,
            expires_at_ms,
            cask_revision,
        )) = header
        else {
            return Ok(None);
        };
        let snapshots = load_plan_items(&connection, id)?;
        let view = RemovalPlanView {
            version: CONTRACT_VERSION,
            id: id.into(),
            cask_id,
            scope,
            category_key,
            state,
            plan_hash: stored_hash,
            created_at_ms,
            expires_at_ms,
            items: snapshots.iter().map(|item| item.view.clone()).collect(),
        };
        if view.state != "INVALIDATED"
            && plan_hash(&view, &snapshots, cask_revision) != view.plan_hash
        {
            return Err("Removal plan integrity mismatch".into());
        }
        Ok(Some(view))
    }
}

impl RemovalInspection for LocalStorage {
    fn verify_managed(&self, cask_root: &str, candidate: &RemovalCandidate) -> Option<String> {
        if candidate.management != "MANAGED" || candidate.containment != "INTERNAL" {
            return None;
        }
        let kind = ResourceKind::from_code(&candidate.kind)?;
        let root = Path::new(cask_root);
        let target = Path::new(&candidate.path);
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
            transaction.commit().map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}
