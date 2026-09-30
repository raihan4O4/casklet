//! Versioned inventory export and conservative manifest recovery.
use super::{LocalStorage, SqliteCasks};
use casklet_application::{CaskRepository, resource::ResourceRepository};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fmt::Write,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RESOURCES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportProfile {
    Portable,
    Machine,
}

#[derive(Debug, Clone)]
pub struct ManifestPreview {
    pub cask_id: String,
    pub cask_name: String,
    pub resource_count: usize,
    pub observed_present: usize,
    pub observed_missing: usize,
    pub unverified: usize,
    pub warnings: Vec<String>,
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Missing or invalid {name}"))
}

fn safe_text(s: &str, limit: usize) -> bool {
    !s.is_empty() && s.len() <= limit && !s.chars().any(char::is_control)
}

#[expect(
    clippy::too_many_lines,
    reason = "validation and read-only observations share the same bounded manifest snapshot"
)]
fn validate_manifest(path: &Path) -> Result<(Value, ManifestPreview), String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if u64::try_from(bytes.len()).map_err(|e| e.to_string())? > MAX_MANIFEST_BYTES {
        return Err("Manifest exceeds 16 MiB limit".into());
    }
    let mut expected = String::new();
    fs::File::open(path.with_file_name("manifest.sha256"))
        .map_err(|e| e.to_string())?
        .take(129)
        .read_to_string(&mut expected)
        .map_err(|e| e.to_string())?;
    if expected.len() > 128 {
        return Err("Invalid manifest checksum".into());
    }
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if expected.trim() != actual {
        return Err("Manifest checksum mismatch".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(1) {
        return Err("Unsupported manifest version".into());
    }
    let cask = value.get("cask").ok_or("Missing cask")?;
    let id = field(cask, "id")?;
    Uuid::parse_str(id).map_err(|_| "Invalid Cask ID")?;
    let name = field(cask, "name")?;
    if !safe_text(name, 120) {
        return Err("Invalid Cask name".into());
    }
    if let Some(description) = cask.get("description") {
        let description = description.as_str().ok_or("Invalid Cask description")?;
        if description.chars().count() > 500 || description.chars().any(char::is_control) {
            return Err("Invalid Cask description".into());
        }
    }
    let resources = value
        .get("resources")
        .and_then(Value::as_array)
        .ok_or("Missing resources")?;
    if let Some(categories) = value.get("categories") {
        let categories = categories.as_array().ok_or("Invalid categories")?;
        if categories.len() > 1_000 {
            return Err("Too many categories".into());
        }
        let mut keys = HashSet::new();
        for category in categories {
            let key = field(category, "key")?;
            if !safe_text(key, 128)
                || !keys.insert(key)
                || !safe_text(field(category, "name")?, 120)
            {
                return Err("Invalid or duplicate category".into());
            }
            if category.get("isBuiltin").and_then(Value::as_bool).is_none() {
                return Err("Invalid category kind".into());
            }
        }
    }
    if resources.len() > MAX_RESOURCES {
        return Err("Too many manifest resources".into());
    }
    if let Some(applications) = value.get("applications") {
        let applications = applications.as_array().ok_or("Invalid applications")?;
        if applications.len() > MAX_RESOURCES {
            return Err("Too many manifest applications".into());
        }
        let mut installed_ids = HashSet::new();
        for app in applications {
            Uuid::parse_str(field(app, "applicationId")?).map_err(|_| "Invalid application ID")?;
            let installation_id = field(app, "installationId")?;
            Uuid::parse_str(installation_id).map_err(|_| "Invalid installation ID")?;
            if !installed_ids.insert(installation_id)
                || !safe_text(field(app, "name")?, 240)
                || !safe_text(field(app, "sourceKey")?, 2048)
                || !matches!(field(app, "scope")?, "USER" | "SYSTEM")
                || field(app, "provenance")? != "DETECTED_WINDOWS_REGISTRY"
                || field(app, "removalPolicy")? != "DETACH_ONLY"
            {
                return Err("Invalid or duplicate application association".into());
            }
            for (key, max) in [
                ("publisher", 240),
                ("displayVersion", 120),
                ("installLocation", 4096),
            ] {
                if let Some(text) = app.get(key).and_then(Value::as_str)
                    && !safe_text(text, max)
                {
                    return Err("Invalid application metadata".into());
                }
            }
        }
    }
    let mut ids = HashSet::new();
    for resource in resources {
        let resource_id = field(resource, "id")?;
        Uuid::parse_str(resource_id).map_err(|_| "Invalid resource ID")?;
        if !ids.insert(resource_id) {
            return Err("Duplicate resource ID".into());
        }
        if !safe_text(field(resource, "name")?, 255) {
            return Err("Invalid resource name".into());
        }
        if !matches!(field(resource, "kind")?, "FILE" | "DIRECTORY") {
            return Err("Invalid resource kind".into());
        }
        if !matches!(field(resource, "management")?, "MANAGED" | "REFERENCED") {
            return Err("Invalid management code".into());
        }
        if !matches!(field(resource, "containment")?, "INTERNAL" | "EXTERNAL") {
            return Err("Invalid containment code".into());
        }
        let path = field(resource, "path")?;
        if path.len() > 4096 || path.contains('\0') {
            return Err("Invalid resource path".into());
        }
        field(resource, "categoryKey")?;
        if let Some(size) = resource.get("sizeBytes")
            && !size.is_null()
            && size.as_u64().is_none()
        {
            return Err("Invalid size".into());
        }
    }
    let cask_root = path.parent().and_then(Path::parent);
    let mut observed_present = 0;
    let mut observed_missing = 0;
    let mut unverified = 0;
    for resource in resources {
        let Some(root) = cask_root else {
            unverified += 1;
            continue;
        };
        let resource_path = Path::new(field(resource, "path")?);
        if resource["containment"] != "INTERNAL" || !resource_path.starts_with(root.join("content"))
        {
            unverified += 1;
            continue;
        }
        match fs::symlink_metadata(resource_path) {
            Ok(metadata)
                if !metadata.file_type().is_symlink()
                    && ((resource["kind"] == "FILE" && metadata.is_file())
                        || (resource["kind"] == "DIRECTORY" && metadata.is_dir())) =>
            {
                observed_present += 1;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => observed_missing += 1,
            _ => unverified += 1,
        }
    }
    let preview = ManifestPreview {
        cask_id: id.into(), cask_name: name.into(), resource_count: resources.len(),
        observed_present, observed_missing, unverified,
        warnings: vec!["Manifest metadata is untrusted. Recovery records all resources as references with unknown availability and detach-only policy.".into()],
    };
    Ok((value, preview))
}

/// # Errors
/// Rejects unreadable, oversized, corrupt, or unsupported manifests.
pub fn preview_manifest(path: &Path) -> Result<ManifestPreview, String> {
    validate_manifest(path).map(|(_, preview)| preview)
}

/// Finds only immediate UUID-named Cask directories in configured local storage.
/// # Errors
/// Rejects an unreadable root, more than 1,000 candidates, or any invalid manifest.
pub fn discover_manifests(
    storage: &LocalStorage,
) -> Result<Vec<(PathBuf, ManifestPreview)>, String> {
    if !storage.root.exists() {
        return Ok(Vec::new());
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(&storage.root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if Uuid::parse_str(name).is_err() {
            continue;
        }
        let manifest = entry.path().join(".casklet").join("manifest.json");
        if !manifest.is_file() {
            continue;
        }
        if found.len() >= 1_000 {
            return Err("Too many Cask manifests".into());
        }
        let preview = preview_manifest(&manifest)?;
        if preview.cask_id != name {
            return Err("Manifest ID differs from Cask directory".into());
        }
        found.push((manifest, preview));
    }
    found.sort_by(|a, b| a.1.cask_id.cmp(&b.1.cask_id));
    Ok(found)
}

#[must_use]
pub fn describe_manifests(items: &[(PathBuf, ManifestPreview)]) -> String {
    let mut result = String::new();
    for (_, preview) in items {
        writeln!(
            result,
            "{}: {} resources ({} present, {} missing, {} unverified)",
            preview.cask_name,
            preview.resource_count,
            preview.observed_present,
            preview.observed_missing,
            preview.unverified
        )
        .expect("writing to String cannot fail");
    }
    result
}

impl SqliteCasks {
    /// Recovers every missing Cask under configured storage. The command can be retried after a partial I/O failure.
    /// # Errors
    /// Rejects invalid manifests before changing the database; database failures may leave earlier Casks recovered.
    pub fn recover_discovered(
        &self,
        storage: &LocalStorage,
    ) -> Result<Vec<ManifestPreview>, String> {
        let manifests = discover_manifests(storage)?;
        let mut recovered = Vec::new();
        for (path, preview) in manifests {
            if self.get(&preview.cask_id)?.is_none() {
                recovered.push(self.recover_manifest(storage, &path)?);
            }
        }
        Ok(recovered)
    }
    /// Returns a versioned, privacy-safe inventory. Machine exports explicitly include local IDs and paths.
    /// # Errors
    /// Returns a database or serialization error.
    pub fn export_inventory(&self, cask_id: &str, profile: ExportProfile) -> Result<Value, String> {
        let cask = self.get(cask_id)?.ok_or("Cask not found")?;
        let mut resources = self.list_resources(cask_id)?;
        resources.sort_by(|a, b| a.id.cmp(&b.id));
        let mut categories = self.list_categories(cask_id)?;
        categories.sort_by(|a, b| a.key.cmp(&b.key));
        let entries: Vec<Value> = resources.iter().map(|r| {
            let mut item = json!({"name":r.name,"kind":r.kind,"management":r.management,
                "containment":r.containment,"availability":r.availability,"categoryKey":r.category_key,
                "sizeBytes":r.size_bytes,"sizeMeasurement":r.size_measurement});
            if profile == ExportProfile::Machine {
                item["id"] = json!(r.id);
                item["path"] = json!(r.path);
            } else if r.containment == "INTERNAL" {
                let root = Path::new(&cask.managed_storage_root).join("content");
                if let Ok(relative) = Path::new(&r.path).strip_prefix(root) {
                    item["relativePath"] = json!(relative.to_string_lossy());
                }
            }
            item
        }).collect();
        let mut result = json!({"schemaVersion":1,"profile":if profile == ExportProfile::Portable {"portable"} else {"machine"},
            "cask":{"name":cask.name,"description":cask.description,"state":cask.state.code()},
            "categories":categories.iter().map(|c| json!({"key":c.key,"name":c.name})).collect::<Vec<_>>(),
            "resources":entries,
            "warnings":["Inventory is informational and does not grant deletion authority."]});
        if profile == ExportProfile::Machine {
            result["cask"]["id"] = json!(cask.id);
            result["cask"]["managedStorageRoot"] = json!(cask.managed_storage_root);
        }
        Ok(result)
    }

    /// # Errors
    /// Returns an inventory read error.
    pub fn export_report(&self, cask_id: &str, profile: ExportProfile) -> Result<String, String> {
        let inventory = self.export_inventory(cask_id, profile)?;
        let name = field(&inventory["cask"], "name")?.replace(['|', '\n', '\r'], " ");
        let mut report = format!(
            "# Cask inventory: {name}\n\nProfile: {}\n\n| Resource | Kind | Management | Category | Size |\n| --- | --- | --- | --- | ---: |\n",
            field(&inventory, "profile")?
        );
        for r in inventory["resources"]
            .as_array()
            .ok_or("Invalid inventory")?
        {
            let clean = |key| field(r, key).unwrap_or("").replace(['|', '\n', '\r'], " ");
            let size = r["sizeBytes"]
                .as_u64()
                .map_or_else(|| "unknown".into(), |n| n.to_string());
            writeln!(
                report,
                "| {} | {} | {} | {} | {} |",
                clean("name"),
                clean("kind"),
                clean("management"),
                clean("categoryKey"),
                size
            )
            .map_err(|e| e.to_string())?;
        }
        report.push_str("\nThis report is informational and grants no cleanup authority.\n");
        Ok(report)
    }

    /// Rebuilds a missing Cask from a checked manifest in the configured managed root.
    /// Every recovered association is detach-only and every location is unverified.
    /// # Errors
    /// Rejects corrupt, incompatible, misplaced, or conflicting manifests and database failures.
    pub fn recover_manifest(
        &self,
        storage: &LocalStorage,
        manifest_path: &Path,
    ) -> Result<ManifestPreview, String> {
        let (manifest, preview) = validate_manifest(manifest_path)?;
        let root = storage.root.join(&preview.cask_id);
        let expected = root.join(".casklet").join("manifest.json");
        if manifest_path.canonicalize().map_err(|e| e.to_string())?
            != expected.canonicalize().map_err(|e| e.to_string())?
        {
            return Err("Manifest is outside the configured Cask root".into());
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let exists: i64 = transaction
            .query_row(
                "SELECT COUNT(*) FROM casks WHERE id=?1",
                [&preview.cask_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if exists != 0 {
            return Err("Cask already exists; current state was not changed".into());
        }
        let created = manifest["cask"]["createdAtMs"]
            .as_i64()
            .filter(|n| *n >= 0)
            .ok_or("Invalid creation time")?;
        transaction.execute("INSERT INTO casks(id,name,description,storage_key,managed_storage_root,created_at_ms,updated_at_ms,revision,state) VALUES (?1,?2,?5,?1,?3,?4,?4,0,'RECOVERY_REQUIRED')",
            params![preview.cask_id,preview.cask_name,root.to_string_lossy(),created,manifest["cask"]["description"].as_str().unwrap_or("")]).map_err(|e|e.to_string())?;
        if let Some(categories) = manifest["categories"].as_array() {
            for category in categories {
                if category["isBuiltin"] == false {
                    let key = field(category, "key")?;
                    Uuid::parse_str(key).map_err(|_| "Invalid custom category key")?;
                    transaction.execute("INSERT INTO categories(key,cask_id,display_name,is_builtin,sort_order) VALUES (?1,?2,?3,0,100)",
                        params![key,preview.cask_id,field(category,"name")?]).map_err(|e|e.to_string())?;
                }
            }
        }
        let resources = manifest["resources"]
            .as_array()
            .ok_or("Missing resources")?;
        for r in resources {
            let id = field(r, "id")?;
            let path = field(r, "path")?;
            let category = field(r, "categoryKey")?;
            let category_exists: i64 = transaction.query_row("SELECT COUNT(*) FROM categories WHERE key=?1 AND (cask_id IS NULL OR cask_id=?2)",params![category,preview.cask_id], |row|row.get(0)).map_err(|e|e.to_string())?;
            let category = if category_exists == 0 {
                "other"
            } else {
                category
            };
            let size = r["sizeBytes"].as_u64().and_then(|n| i64::try_from(n).ok());
            let existing: Option<(String,String,String)> = transaction.query_row(
                "SELECT r.kind,r.display_name,l.path_display FROM resources r JOIN resource_locations l ON l.resource_id=r.id AND l.is_primary=1 AND l.retired_at_ms IS NULL WHERE r.id=?1",
                [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))
            ).optional().map_err(|e|e.to_string())?;
            if let Some((kind, name, existing_path)) = existing {
                if kind != field(r, "kind")? || name != field(r, "name")? || existing_path != path {
                    return Err("Shared resource ID conflicts with existing metadata".into());
                }
            } else {
                transaction.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms) VALUES (?1,?2,?3,'REFERENCED','MANIFEST_RECOVERY','UNKNOWN',?4,'UNKNOWN',0,?5)",
                    params![id,field(r,"kind")?,field(r,"name")?,size,created]).map_err(|e|e.to_string())?;
                transaction.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms) VALUES (?1,?2,?3,?4,'EXTERNAL',1,?5)",
                    params![Uuid::new_v4().to_string(),id,path,path.to_lowercase(),created]).map_err(|e|e.to_string())?;
            }
            transaction.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,?4,'REFERENCED','DETACH_ONLY',?5)",
                params![Uuid::new_v4().to_string(),preview.cask_id,id,category,created]).map_err(|e|e.to_string())?;
        }
        if let Some(applications) = manifest.get("applications").and_then(Value::as_array) {
            for app in applications {
                let application_id = field(app, "applicationId")?;
                let installation_id = field(app, "installationId")?;
                let source_key = field(app, "sourceKey")?;
                let existing: Option<String> = transaction
                    .query_row(
                        "SELECT source_key FROM windows_installations WHERE id=?1",
                        [installation_id],
                        |r| r.get(0),
                    )
                    .optional()
                    .map_err(|e| e.to_string())?;
                if let Some(existing) = existing
                    && existing != source_key
                {
                    return Err("Recovered installation ID conflicts with existing source".into());
                }
                transaction.execute("INSERT INTO applications(id,canonical_name,publisher,created_at_ms) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO NOTHING",params![application_id,field(app,"name")?,app["publisher"].as_str(),created]).map_err(|e|e.to_string())?;
                let evidence = serde_json::to_string(app).map_err(|e| e.to_string())?;
                let hash = format!("{:x}", Sha256::digest(evidence.as_bytes()));
                transaction.execute("INSERT INTO windows_installations(id,application_id,source_key,scope,display_name,publisher,display_version,install_location,evidence_json,evidence_sha256,state,first_seen_at_ms,last_seen_at_ms,last_verified_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'AMBIGUOUS',?11,?11,0) ON CONFLICT(id) DO NOTHING",
                    params![installation_id,application_id,source_key,field(app,"scope")?,field(app,"name")?,app["publisher"].as_str(),app["displayVersion"].as_str(),app["installLocation"].as_str(),evidence,hash,created]).map_err(|e|e.to_string())?;
                transaction.execute("INSERT INTO windows_cask_associations(id,cask_id,installation_id,created_at_ms) VALUES (?1,?2,?3,?4)",params![Uuid::new_v4().to_string(),preview.cask_id,installation_id,created]).map_err(|e|e.to_string())?;
            }
        }
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(preview)
    }
}

/// # Errors
/// Rejects oversized exports and existing output paths or returns an I/O error.
pub fn write_export(path: &Path, content: &[u8]) -> Result<(), String> {
    if u64::try_from(content.len()).map_err(|e| e.to_string())? > MAX_MANIFEST_BYTES {
        return Err("Export exceeds 16 MiB limit".into());
    }
    let parent: PathBuf = path
        .parent()
        .ok_or("Output path needs a parent")?
        .to_path_buf();
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let temporary = parent.join(format!(".casklet-export-{}.tmp", Uuid::new_v4()));
    fs::write(&temporary, content).map_err(|e| e.to_string())?;
    if path.exists() {
        fs::remove_file(&temporary).ok();
        return Err("Output already exists".into());
    }
    fs::rename(&temporary, path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::CaskService;
    use casklet_contracts::{CONTRACT_VERSION, CreateCask};

    #[test]
    fn portable_export_omits_machine_identity_and_recovery_preserves_ownership() {
        let root = std::env::temp_dir().join(format!("casklet-m5-{}", Uuid::new_v4()));
        let storage = LocalStorage::new(root.join("casks"));
        let source = CaskService {
            repository: SqliteCasks::open(root.join("source.db")).unwrap(),
            storage: storage.clone(),
        };
        let cask = source
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Test".into(),
                },
                "test",
            )
            .unwrap();
        let portable = source
            .repository
            .export_inventory(&cask.id, ExportProfile::Portable)
            .unwrap();
        assert_eq!(portable["schemaVersion"], 1);
        assert!(portable["cask"].get("id").is_none());
        assert!(portable["cask"].get("managedStorageRoot").is_none());
        let machine = source
            .repository
            .export_inventory(&cask.id, ExportProfile::Machine)
            .unwrap();
        assert_eq!(machine["cask"]["id"], cask.id);
        let manifest = root
            .join("casks")
            .join(&cask.id)
            .join(".casklet")
            .join("manifest.json");
        let target = SqliteCasks::open(root.join("target.db")).unwrap();
        let preview = target.recover_manifest(&storage, &manifest).unwrap();
        assert_eq!(preview.resource_count, 0);
        assert_eq!(
            target.get(&cask.id).unwrap().unwrap().state.code(),
            "RECOVERY_REQUIRED"
        );
        let before = target.get(&cask.id).unwrap().unwrap();
        fs::write(manifest.with_file_name("manifest.sha256"), "wrong").unwrap();
        assert!(preview_manifest(&manifest).is_err());
        assert_eq!(
            target.get(&cask.id).unwrap().unwrap().revision,
            before.revision
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn several_manifests_reassociate_shared_resource_without_cleanup_authority() {
        let root = std::env::temp_dir().join(format!("casklet-m5-share-{}", Uuid::new_v4()));
        let storage = LocalStorage::new(root.join("casks"));
        let database = SqliteCasks::open(root.join("state.db")).unwrap();
        let shared = Uuid::new_v4().to_string();
        for index in 0..2 {
            let cask_id = Uuid::new_v4().to_string();
            let metadata = root.join("casks").join(&cask_id).join(".casklet");
            fs::create_dir_all(&metadata).unwrap();
            let manifest = json!({"schemaVersion":1,
                "cask":{"id":cask_id,"name":format!("Cask {index}"),"createdAtMs":1},
                "resources":[{"id":shared,"name":"Shared.txt","kind":"FILE",
                    "management":"MANAGED","containment":"EXTERNAL","path":"C:\\private\\Shared.txt",
                    "categoryKey":"other","sizeBytes":4}]});
            let bytes = serde_json::to_vec(&manifest).unwrap();
            fs::write(metadata.join("manifest.json"), &bytes).unwrap();
            fs::write(
                metadata.join("manifest.sha256"),
                format!("{:x}", Sha256::digest(&bytes)),
            )
            .unwrap();
        }
        let previews = discover_manifests(&storage).unwrap();
        assert_eq!(previews.len(), 2);
        assert_eq!(previews[0].1.unverified, 1);
        let checksum = previews[1].0.with_file_name("manifest.sha256");
        let original_checksum = fs::read(&checksum).unwrap();
        fs::write(&checksum, "truncated").unwrap();
        assert!(database.recover_discovered(&storage).is_err());
        assert!(database.list().unwrap().is_empty());
        fs::write(&checksum, original_checksum).unwrap();
        assert_eq!(database.recover_discovered(&storage).unwrap().len(), 2);
        assert!(database.recover_discovered(&storage).unwrap().is_empty());
        let connection = database.connection().unwrap();
        let actual: (String,String,i64) = connection.query_row(
            "SELECT r.management,r.availability,COUNT(a.id) FROM resources r JOIN resource_associations a ON a.resource_id=r.id WHERE r.id=?1 AND a.removal_policy='DETACH_ONLY' GROUP BY r.id",
            [&shared],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
        assert_eq!(actual, ("REFERENCED".into(), "UNKNOWN".into(), 2));
        drop(connection);
        fs::remove_dir_all(root).unwrap();
    }
}
