//! SQLite resource, category, and intake-intent persistence.
use super::SqliteCasks;
use casklet_application::resource::ResourceRepository;
use casklet_contracts::{CONTRACT_VERSION, CategoryView, ResourceView};
use casklet_domain::resource::{IntakeMode, Resource};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

pub(super) fn reject_pending_intake_overlap(
    connection: &rusqlite::Connection,
    path: &str,
) -> Result<(), String> {
    let mut statement = connection.prepare("SELECT i.source_path,i.staging_path,i.final_path FROM intake_operations i JOIN operation_locks lock ON lock.operation_id=i.id").map_err(|e| e.to_string())?;
    for row in statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (source, staging, destination) = row.map_err(|e| e.to_string())?;
        if [source, staging, destination]
            .iter()
            .any(|other| super::removal::paths_overlap(path, other))
        {
            return Err("This location overlaps an active or unrecovered intake".into());
        }
    }
    Ok(())
}

fn reject_locked_overlap(connection: &rusqlite::Connection, path: &str) -> Result<(), String> {
    if std::path::Path::new(path).components().any(|part| {
        part.as_os_str()
            .to_string_lossy()
            .to_lowercase()
            .starts_with(".casklet-removal-")
    }) {
        return Err(
            "Recovery quarantine must be inspected and restored through recovery, not intake"
                .into(),
        );
    }
    let mut statement = connection.prepare("SELECT l.path_display FROM resource_locations l JOIN resource_associations a ON a.resource_id=l.resource_id AND a.removed_at_ms IS NULL JOIN removal_locks lock ON lock.cask_id=a.cask_id WHERE l.is_primary=1 AND l.retired_at_ms IS NULL").map_err(|e| e.to_string())?;
    for row in statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
    {
        if super::removal::paths_overlap(path, &row.map_err(|e| e.to_string())?) {
            return Err("This location is locked by an active or uncertain cleanup".into());
        }
    }
    Ok(())
}

const SELECT_RESOURCE: &str = "SELECT r.id,a.cask_id,r.display_name,l.path_display,r.kind,r.management,l.containment,r.availability,a.category_key,r.size_bytes,r.size_measurement,r.unknown_entries,(SELECT COUNT(*) > 1 FROM resource_associations shared WHERE shared.resource_id=r.id AND shared.removed_at_ms IS NULL),a.relationship,r.provenance FROM resources r JOIN resource_associations a ON a.resource_id=r.id AND a.removed_at_ms IS NULL JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL";

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
        shared: row.get(12)?,
        ownership: row.get(13)?,
        provenance: row.get(14)?,
        containment: row.get(6)?,
        availability: row.get(7)?,
        category_key: row.get(8)?,
        size_bytes: size.and_then(|n| u64::try_from(n).ok()),
        size_measurement: row.get(10)?,
        unknown_entries: u64::try_from(unknown).unwrap_or(0),
    })
}

// The native object key excludes mutable content/tree digests. It grants no authority.
fn object_key(identity: &str) -> Option<String> {
    if let Some(tree) = identity.strip_prefix("tree-v2:") {
        return tree.split_once('|').map(|(root, _)| root.to_string());
    }
    let parts: Vec<_> = identity.split(':').collect();
    (parts.len() == 4
        && parts[..3]
            .iter()
            .all(|part| part.len() == 16 && part.bytes().all(|byte| byte.is_ascii_hexdigit())))
    .then(|| parts[..3].join(":"))
}

// Canonical paths come from the storage adapter. Native object keys and the
// service-selected ID handle aliases without merging distinct case-sensitive files.
fn existing_intake_resource(
    connection: &rusqlite::Connection,
    cask_id: &str,
    path: &str,
    identity: Option<&str>,
    requested_id: &str,
) -> Result<Option<ResourceView>, String> {
    let mut statement = connection.prepare(&format!("{SELECT_RESOURCE} WHERE a.cask_id=?1 ORDER BY a.relationship='OWNED' DESC,a.added_at_ms,r.id")).map_err(|e|e.to_string())?;
    let rows = statement
        .query_map([cask_id], resource_row)
        .map_err(|e| e.to_string())?;
    for row in rows {
        let resource = row.map_err(|e| e.to_string())?;
        let recorded: Option<String> = connection.query_row("SELECT identity_key FROM resource_locations WHERE resource_id=?1 AND is_primary=1 AND retired_at_ms IS NULL", [&resource.id], |row|row.get(0)).map_err(|e|e.to_string())?;
        if resource.id == requested_id
            || resource.path == path
            || identity
                .and_then(object_key)
                .zip(recorded.as_deref().and_then(object_key))
                .is_some_and(|(left, right)| left == right)
        {
            return Ok(Some(resource));
        }
    }
    Ok(None)
}

// Repair historical duplicate user references without deleting audit records or bytes.
// Other Casks, ownership grants, and installation artifacts remain dependencies.
fn retire_duplicate_references(
    connection: &rusqlite::Connection,
    keep: &ResourceView,
    now: i64,
) -> Result<(), String> {
    let mut statement = connection.prepare(&format!("{SELECT_RESOURCE} WHERE a.cask_id=?1 AND r.provenance='USER_REFERENCE' AND a.relationship='REFERENCED' AND NOT EXISTS(SELECT 1 FROM installer_artifacts i WHERE i.resource_id=r.id)")).map_err(|e|e.to_string())?;
    let rows = statement
        .query_map([&keep.cask_id], resource_row)
        .map_err(|e| e.to_string())?;
    let mut changed = false;
    for row in rows {
        let resource = row.map_err(|e| e.to_string())?;
        let identities = |id: &str| -> Result<Option<String>, String> {
            connection.query_row("SELECT identity_key FROM resource_locations WHERE resource_id=?1 AND is_primary=1 AND retired_at_ms IS NULL", [id], |row|row.get(0)).map_err(|e|e.to_string())
        };
        let same_object = identities(&resource.id)?
            .as_deref()
            .and_then(object_key)
            .zip(identities(&keep.id)?.as_deref().and_then(object_key))
            .is_some_and(|(left, right)| left == right);
        if resource.id != keep.id && (resource.path == keep.path || same_object) {
            connection.execute("UPDATE resource_associations SET removed_at_ms=?3 WHERE resource_id=?1 AND cask_id=?2 AND removed_at_ms IS NULL", params![resource.id,keep.cask_id,now]).map_err(|e|e.to_string())?;
            changed = true;
        }
    }
    if changed {
        connection.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms", params![keep.cask_id,now]).map_err(|e|e.to_string())?;
    }
    Ok(())
}

impl ResourceRepository for SqliteCasks {
    fn cached_application_sizes(
        &self,
        entries: &[casklet_application::resource::ApplicationStorageEntry],
    ) -> Result<Vec<casklet_application::resource::ApplicationSize>, String> {
        use casklet_application::resource::ApplicationSize;
        let connection = self.connection()?;
        let mut query = connection.prepare("SELECT w.state,s.size_bytes,s.estimated FROM windows_installations w LEFT JOIN application_size_snapshots s ON s.source_key=w.source_key COLLATE NOCASE AND s.evidence_sha256=w.evidence_sha256 WHERE w.source_key=?1 COLLATE NOCASE").map_err(|e| e.to_string())?;
        entries
            .iter()
            .map(|entry| {
                query
                    .query_row([&entry.source_key], |r| {
                        let state: String = r.get(0)?;
                        let bytes: Option<i64> = r.get(1)?;
                        let bytes = bytes.and_then(|value| u64::try_from(value).ok());
                        let estimated: Option<bool> = r.get(2)?;
                        Ok(if state == "MISSING" {
                            ApplicationSize::Missing
                        } else if state == "PRESENT" {
                            bytes.map_or(ApplicationSize::Unknown, |bytes| ApplicationSize::Known {
                                bytes,
                                estimated: estimated.unwrap_or(true),
                            })
                        } else {
                            ApplicationSize::Unknown
                        })
                    })
                    .optional()
                    .map(|value| value.unwrap_or(ApplicationSize::Unknown))
                    .map_err(|e| e.to_string())
            })
            .collect()
    }
    fn record_application_sizes(
        &self,
        entries: &[casklet_application::resource::ApplicationStorageEntry],
        sizes: &[casklet_application::resource::ApplicationSize],
    ) -> Result<(), String> {
        use casklet_application::resource::ApplicationSize;
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        for (entry, size) in entries.iter().zip(sizes) {
            let (bytes, estimated) = match size {
                ApplicationSize::Known { bytes, estimated } => {
                    (i64::try_from(*bytes).ok(), *estimated)
                }
                _ => (None, false),
            };
            tx.execute("INSERT INTO application_size_snapshots(source_key,evidence_sha256,size_bytes,estimated,measured_at_ms) SELECT source_key,evidence_sha256,?2,?3,CAST(strftime('%s','now') AS INTEGER)*1000 FROM windows_installations WHERE source_key=?1 COLLATE NOCASE AND evidence_sha256=?4 ON CONFLICT(source_key) DO UPDATE SET evidence_sha256=excluded.evidence_sha256,size_bytes=excluded.size_bytes,estimated=excluded.estimated,measured_at_ms=excluded.measured_at_ms", params![entry.source_key,bytes,estimated,entry.evidence_sha256]).map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    fn recorded_resource_identity(&self, id: &str) -> Result<Option<String>, String> {
        self.connection()?.query_row("SELECT identity_key FROM resource_locations WHERE resource_id=?1 AND is_primary=1 AND retired_at_ms IS NULL", [id], |row|row.get(0)).optional().map(Option::flatten).map_err(|e|e.to_string())
    }
    fn record_measurement(&self, resource: &ResourceView) -> Result<(), String> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let changed = transaction.execute("UPDATE resources SET availability=?2,size_bytes=?3,size_measurement=?4,unknown_entries=?5 WHERE id=?1 AND EXISTS(SELECT 1 FROM resource_locations l WHERE l.resource_id=?1 AND l.path_display=?6 AND l.is_primary=1 AND l.retired_at_ms IS NULL) AND (availability<>?2 OR size_bytes IS NOT ?3 OR size_measurement<>?4 OR unknown_entries<>?5)",
            params![resource.id,resource.availability,resource.size_bytes.map(i64::try_from).transpose().map_err(|e|e.to_string())?,resource.size_measurement,i64::try_from(resource.unknown_entries).map_err(|e|e.to_string())?,resource.path]).map_err(|e|e.to_string())?;
        if changed != 0 {
            transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) SELECT cask_id,1,'PENDING',CAST(unixepoch('subsec')*1000 AS INTEGER) FROM resource_associations WHERE resource_id=?1 AND removed_at_ms IS NULL ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",[&resource.id]).map_err(|e|e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }
    fn application_storage_entries(
        &self,
        cask_id: &str,
    ) -> Result<Vec<casklet_application::resource::ApplicationStorageEntry>, String> {
        let connection = self.connection()?;
        let mut query = connection.prepare("SELECT w.source_key,w.evidence_sha256, EXISTS(SELECT 1 FROM managed_msi_installations m WHERE m.installation_id=w.id AND m.cask_id=a.cask_id AND m.state='OWNED') FROM windows_cask_associations a JOIN windows_installations w ON w.id=a.installation_id WHERE a.cask_id=?1 AND a.removed_at_ms IS NULL UNION ALL SELECT w.source_key,w.evidence_sha256,1 FROM winget_installations g JOIN windows_installations w ON w.id=g.registry_installation_id WHERE g.cask_id=?1 AND g.state IN ('INSTALLED','CHANGED','RECOVERY_REQUIRED') ORDER BY source_key").map_err(|e| e.to_string())?;
        let mut entries: Vec<casklet_application::resource::ApplicationStorageEntry> = Vec::new();
        for row in query
            .query_map([cask_id], |row| {
                Ok(casklet_application::resource::ApplicationStorageEntry {
                    source_key: row.get(0)?,
                    evidence_sha256: row.get(1)?,
                    owned: row.get(2)?,
                })
            })
            .map_err(|e| e.to_string())?
        {
            let entry = row.map_err(|e| e.to_string())?;
            let alias =
                casklet_application::windows_inventory::user_registration_alias(&entry.source_key);
            if let Some(existing) = entries.iter_mut().find(|other| {
                other.source_key.eq_ignore_ascii_case(&entry.source_key)
                    || alias
                        .as_ref()
                        .is_some_and(|key| key.eq_ignore_ascii_case(&other.source_key))
            }) {
                existing.owned |= entry.owned;
            } else {
                entries.push(entry);
            }
        }
        Ok(entries)
    }
    fn installer_resource_ids(&self, cask_id: &str) -> Result<Vec<String>, String> {
        let connection = self.connection()?;
        let mut query = connection
            .prepare("SELECT resource_id FROM installer_artifacts WHERE cask_id=?1")
            .map_err(|e| e.to_string())?;
        query
            .query_map([cask_id], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn commit_adoption(
        &self,
        resource: &Resource,
        expected: Option<&str>,
    ) -> Result<String, String> {
        let mut resource = resource.clone();
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        reject_locked_overlap(&transaction, &resource.path)?;
        reject_pending_intake_overlap(&transaction, &resource.path)?;
        if let Some(parent) = self.db_path.parent()
            && let Ok(state_path) = std::fs::canonicalize(parent)
            && super::removal::paths_overlap(&resource.path, &state_path.to_string_lossy())
        {
            return Err("The database directory cannot be owned as resource data".into());
        }
        let available: i64 = transaction.query_row("SELECT COUNT(*) FROM casks WHERE id=?1 AND state='ACTIVE' AND NOT EXISTS (SELECT 1 FROM operation_locks WHERE lock_key='cask:'||?1||':mutation') AND NOT EXISTS (SELECT 1 FROM removal_locks WHERE cask_id=?1) AND NOT EXISTS (SELECT 1 FROM installer_locks WHERE cask_id=?1) AND NOT EXISTS (SELECT 1 FROM msi_cleanup_locks WHERE cask_id=?1)", [&resource.cask_id], |row| row.get(0)).map_err(|e| e.to_string())?;
        if available != 1 {
            return Err("Cask changed or has unfinished work".into());
        }
        if let Some(existing) = existing_intake_resource(
            &transaction,
            &resource.cask_id,
            &resource.path,
            resource.identity_key.as_deref(),
            &resource.id,
        )? {
            if existing.kind != resource.kind.code() {
                return Err(
                    "The registered item changed type; review its existing association first"
                        .into(),
                );
            }
            if let Some(expected) = expected {
                if existing.id != resource.id || existing.ownership != expected {
                    return Err("Ownership changed; refresh this item and try again".into());
                }
            } else {
                retire_duplicate_references(&transaction, &existing, resource.created_at_ms)?;
                transaction.commit().map_err(|e| e.to_string())?;
                return Ok(existing.id);
            }
            resource.id.clone_from(&existing.id);
            resource.path.clone_from(&existing.path);
            retire_duplicate_references(&transaction, &existing, resource.created_at_ms)?;
        }
        if super::removal::overlap_count(
            &transaction,
            &resource.id,
            &resource.cask_id,
            &resource.path,
        )? != 0
        {
            return Err("This path overlaps a registered resource; keep it referenced or resolve its associations first".into());
        }
        if expected.is_some() && !transaction.query_row("SELECT EXISTS(SELECT 1 FROM resource_associations WHERE cask_id=?1 AND resource_id=?2 AND removed_at_ms IS NULL)", params![resource.cask_id,resource.id], |row|row.get::<_,bool>(0)).map_err(|e|e.to_string())? {
            return Err("Item is no longer associated with this Cask".into());
        }
        let identity = resource
            .identity_key
            .as_ref()
            .ok_or("Ownership identity missing")?;
        let renewing: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1)",
                [&resource.id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if renewing {
            let eligible: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL JOIN resource_associations a ON a.resource_id=r.id AND a.removed_at_ms IS NULL JOIN resource_ownership_grants g ON g.resource_id=r.id AND g.cask_id=a.cask_id AND g.exact_path=l.path_display WHERE r.id=?1 AND a.cask_id=?2 AND l.path_display=?3 AND r.management='MANAGED' AND r.provenance='USER_ADOPTED' AND l.containment='EXTERNAL' AND a.relationship='OWNED')", params![resource.id,resource.cask_id,resource.path], |row|row.get(0)).map_err(|e|e.to_string())?;
            let reference: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL JOIN resource_associations a ON a.resource_id=r.id AND a.removed_at_ms IS NULL WHERE r.id=?1 AND a.cask_id=?2 AND l.path_display=?3 AND r.management='REFERENCED' AND r.provenance IN ('USER_REFERENCE','USER_ADOPTED') AND l.containment='EXTERNAL' AND a.relationship='REFERENCED' AND NOT EXISTS(SELECT 1 FROM installer_artifacts i WHERE i.resource_id=r.id))", params![resource.id,resource.cask_id,resource.path], |row|row.get(0)).map_err(|e|e.to_string())?;
            if !eligible && !reference {
                return Err("This item cannot be converted to in-place ownership".into());
            }
            if reference {
                transaction.execute("UPDATE resources SET management='MANAGED',provenance='USER_ADOPTED' WHERE id=?1", [&resource.id]).map_err(|e|e.to_string())?;
                transaction.execute("UPDATE resource_associations SET relationship='OWNED',removal_policy='DELETE_WHEN_UNREFERENCED',revision=revision+1 WHERE resource_id=?1 AND cask_id=?2 AND removed_at_ms IS NULL", params![resource.id,resource.cask_id]).map_err(|e|e.to_string())?;
                transaction.execute("INSERT INTO resource_ownership_grants(resource_id,cask_id,exact_path,identity_key,consent_at_ms) VALUES (?1,?2,?3,?4,?5)", params![resource.id,resource.cask_id,resource.path,identity,resource.created_at_ms]).map_err(|e|e.to_string())?;
            }
            transaction.execute("UPDATE resources SET availability='PRESENT',size_bytes=?2,size_measurement=?3,unknown_entries=0,revision=revision+1 WHERE id=?1",params![resource.id,resource.size_bytes.map(i64::try_from).transpose().map_err(|e|e.to_string())?,resource.size_measurement]).map_err(|e|e.to_string())?;
            transaction.execute("UPDATE resource_locations SET identity_key=?2 WHERE resource_id=?1 AND is_primary=1 AND retired_at_ms IS NULL",params![resource.id,identity]).map_err(|e|e.to_string())?;
            transaction.execute("UPDATE resource_ownership_grants SET identity_key=?2,consent_at_ms=?3 WHERE resource_id=?1",params![resource.id,identity,resource.created_at_ms]).map_err(|e|e.to_string())?;
        } else {
            transaction.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms) VALUES (?1,?2,?3,'MANAGED','USER_ADOPTED','PRESENT',?4,?5,0,?6)", params![resource.id,resource.kind.code(),resource.name,resource.size_bytes.map(i64::try_from).transpose().map_err(|e| e.to_string())?,resource.size_measurement,resource.created_at_ms]).map_err(|e| e.to_string())?;
            transaction.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms,identity_key) VALUES (?1,?2,?3,?4,'EXTERNAL',1,?5,?6)", params![Uuid::new_v4().to_string(),resource.id,resource.path,resource.path.to_lowercase(),resource.created_at_ms,identity]).map_err(|e| e.to_string())?;
            transaction.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,?4,'OWNED','DELETE_WHEN_UNREFERENCED',?5)", params![Uuid::new_v4().to_string(),resource.cask_id,resource.id,resource.category_key,resource.created_at_ms]).map_err(|e| e.to_string())?;
            transaction.execute("INSERT INTO resource_ownership_grants(resource_id,cask_id,exact_path,identity_key,consent_at_ms) VALUES (?1,?2,?3,?4,?5)", params![resource.id,resource.cask_id,resource.path,identity,resource.created_at_ms]).map_err(|e| e.to_string())?;
        }
        transaction.execute("INSERT INTO operations(id,cask_id,type,state,current_step_code,started_at_ms,updated_at_ms,finished_at_ms) VALUES (?1,?2,'RESOURCE_INTAKE','SUCCEEDED','ADOPT_IN_PLACE',?3,?3,?3)", params![Uuid::new_v4().to_string(),resource.cask_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        transaction.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![resource.cask_id,resource.created_at_ms]).map_err(|e| e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(resource.id.clone())
    }
    fn commit_ownership_policy(
        &self,
        resource: &Resource,
        expected: &str,
        owned: bool,
    ) -> Result<String, String> {
        let current = self
            .get_resource(&resource.id)?
            .ok_or("Item no longer exists")?;
        if owned && current.containment == "EXTERNAL" {
            return self.commit_adoption(resource, Some(expected));
        }
        let mut connection = self.connection()?;
        let tx = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        reject_locked_overlap(&tx, &resource.path)?;
        reject_pending_intake_overlap(&tx, &resource.path)?;
        let eligible: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM resource_associations a JOIN resources r ON r.id=a.resource_id JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL JOIN casks c ON c.id=a.cask_id WHERE a.cask_id=?1 AND r.id=?2 AND a.removed_at_ms IS NULL AND a.relationship=?3 AND l.path_display=?4 AND c.state='ACTIVE' AND NOT EXISTS(SELECT 1 FROM operation_locks WHERE lock_key='cask:'||c.id||':mutation') AND NOT EXISTS(SELECT 1 FROM removal_locks WHERE cask_id=c.id) AND NOT EXISTS(SELECT 1 FROM installer_locks WHERE cask_id=c.id) AND NOT EXISTS(SELECT 1 FROM msi_cleanup_locks WHERE cask_id=c.id) AND NOT EXISTS(SELECT 1 FROM installer_artifacts WHERE resource_id=r.id))", params![resource.cask_id,resource.id,expected,resource.path], |row|row.get(0)).map_err(|e|e.to_string())?;
        if !eligible {
            return Err("Item changed, is an installation artifact, or Cask has unfinished work; refresh and retry".into());
        }
        if owned {
            let internal: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM resources r JOIN resource_locations l ON l.resource_id=r.id WHERE r.id=?1 AND r.provenance='ADDED_BY_CASKLET' AND l.containment='INTERNAL' AND l.is_primary=1 AND l.retired_at_ms IS NULL)", [&resource.id], |row|row.get(0)).map_err(|e|e.to_string())?;
            if !internal
                || super::removal::overlap_count(
                    &tx,
                    &resource.id,
                    &resource.cask_id,
                    &resource.path,
                )? != 0
            {
                return Err(
                    "Internal allocation cannot be owned while shared or overlapping".into(),
                );
            }
            let identity = resource
                .identity_key
                .as_ref()
                .ok_or("Ownership identity unavailable")?;
            tx.execute("UPDATE resource_locations SET identity_key=?2 WHERE resource_id=?1 AND is_primary=1 AND retired_at_ms IS NULL",params![resource.id,identity]).map_err(|e|e.to_string())?;
        }
        let relationship = if owned { "OWNED" } else { "REFERENCED" };
        tx.execute("UPDATE resource_associations SET relationship=?3,removal_policy=?4,revision=revision+1 WHERE resource_id=?1 AND cask_id=?2 AND removed_at_ms IS NULL",params![resource.id,resource.cask_id,relationship,if owned {"DELETE_WHEN_UNREFERENCED"} else {"DETACH_ONLY"}]).map_err(|e|e.to_string())?;
        // A resource shared by other Casks must not change their classification.
        if !tx.query_row("SELECT EXISTS(SELECT 1 FROM resource_associations WHERE resource_id=?1 AND cask_id<>?2 AND removed_at_ms IS NULL)",params![resource.id,resource.cask_id],|r|r.get::<_,bool>(0)).map_err(|e|e.to_string())? {
            tx.execute("UPDATE resources SET management=?2,revision=revision+1 WHERE id=?1",params![resource.id,if owned {"MANAGED"} else {"REFERENCED"}]).map_err(|e|e.to_string())?;
        }
        if !owned {
            tx.execute(
                "DELETE FROM resource_ownership_grants WHERE resource_id=?1 AND cask_id=?2",
                params![resource.id, resource.cask_id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute("INSERT INTO operations(id,cask_id,type,state,current_step_code,started_at_ms,updated_at_ms,finished_at_ms) VALUES (?1,?2,'RESOURCE_INTAKE','SUCCEEDED','CHANGE_OWNERSHIP',?3,?3,?3)",params![Uuid::new_v4().to_string(),resource.cask_id,resource.created_at_ms]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![resource.cask_id,resource.created_at_ms]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(resource.id.clone())
    }
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
        // Revalidate inside the transaction: removal/archive may win after the application's initial lookup.
        let active: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM casks WHERE id=?1 AND state='ACTIVE')",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !active {
            return Err("Cask is no longer active".into());
        }
        reject_locked_overlap(&transaction, source)?;
        let removal_lock: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let installer_lock: i64 = transaction
            .query_row(
                "SELECT (SELECT COUNT(*) FROM installer_locks WHERE cask_id=?1)+(SELECT COUNT(*) FROM msi_cleanup_locks WHERE cask_id=?1)",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if removal_lock != 0 || installer_lock != 0 {
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
    fn commit_resource(&self, resource: &Resource, op_id: Option<&str>) -> Result<String, String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;
        reject_locked_overlap(&transaction, &resource.path)?;
        let active: bool = transaction
            .query_row(
                "SELECT state='ACTIVE' FROM casks WHERE id=?1",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !active {
            return Err("Cask is no longer active".into());
        }
        if op_id.is_none() {
            let locked: i64 = transaction
                .query_row(
                    "SELECT COUNT(*) FROM operation_locks WHERE lock_key=?1",
                    [format!("cask:{}:mutation", resource.cask_id)],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            if locked != 0 {
                return Err("Cask has unfinished intake work".into());
            }
        }
        let locked: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM removal_locks WHERE cask_id=?1",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let installer_lock: i64 = transaction
            .query_row(
                "SELECT (SELECT COUNT(*) FROM installer_locks WHERE cask_id=?1)+(SELECT COUNT(*) FROM msi_cleanup_locks WHERE cask_id=?1)",
                [&resource.cask_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if locked != 0 || installer_lock != 0 {
            return Err("Cask has an active or unrecovered removal".into());
        }
        if op_id.is_none()
            && let Some(existing) = existing_intake_resource(
                &transaction,
                &resource.cask_id,
                &resource.path,
                resource.identity_key.as_deref(),
                &resource.id,
            )?
        {
            retire_duplicate_references(&transaction, &existing, resource.created_at_ms)?;
            transaction.commit().map_err(|e| e.to_string())?;
            return Ok(existing.id);
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
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(resource.id.clone())
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
        let mut resources: Vec<ResourceView> = stmt
            .query_map([cask_id], resource_row)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for resource in &mut resources {
            if resource.management == "MANAGED" {
                resource.shared |= super::removal::overlap_count(
                    &connection,
                    &resource.id,
                    &resource.cask_id,
                    &resource.path,
                )? > 0;
            }
        }
        Ok(resources)
    }
    fn get_resource(&self, id: &str) -> Result<Option<ResourceView>, String> {
        let connection = self.connection()?;
        let mut resource = connection
            .query_row(
                &format!("{SELECT_RESOURCE} WHERE r.id=?1"),
                [id],
                resource_row,
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(resource) = &mut resource {
            resource.shared |= super::removal::overlap_count(
                &connection,
                &resource.id,
                &resource.cask_id,
                &resource.path,
            )? > 0;
        }
        Ok(resource)
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
