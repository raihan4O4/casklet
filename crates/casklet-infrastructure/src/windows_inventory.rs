//! Windows uninstall-registration inventory. Registry metadata is evidence, never ownership.
use super::{LocalStorage, SqliteCasks};
use casklet_application::windows_inventory::{
    InstalledInventoryRepository, InstalledInventorySource, ObservedInstallation, observed_view,
};
use casklet_contracts::{CONTRACT_VERSION, DetectedApplicationView};
use rusqlite::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRegistration {
    source_key: String,
    scope: String,
    display_name: Option<String>,
    publisher: Option<String>,
    display_version: Option<String>,
    install_location: Option<String>,
    windows_installer: Option<u32>,
    system_component: Option<u32>,
    parent_key_name: Option<String>,
}

fn trimmed(value: Option<String>, max: usize) -> Option<String> {
    value
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && v.chars().count() <= max && !v.chars().any(char::is_control))
}

fn normalize(raw: RawRegistration) -> Option<ObservedInstallation> {
    if raw.system_component == Some(1) || raw.parent_key_name.is_some() {
        return None;
    }
    let name = trimmed(raw.display_name.clone(), 240)?;
    let publisher = trimmed(raw.publisher.clone(), 240);
    let display_version = trimmed(raw.display_version.clone(), 120);
    let install_location = trimmed(raw.install_location.clone(), 4096);
    if raw.source_key.len() > 2048 || !matches!(raw.scope.as_str(), "USER" | "SYSTEM") {
        return None;
    }
    let source_key = raw.source_key;
    let application_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, source_key.as_bytes()).to_string();
    let installation_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, source_key.as_bytes()).to_string();
    let evidence_json = serde_json::to_string(&RawRegistration {
        source_key: source_key.clone(),
        scope: raw.scope.clone(),
        display_name: raw.display_name,
        publisher: raw.publisher,
        display_version: raw.display_version,
        install_location: raw.install_location,
        windows_installer: raw.windows_installer,
        system_component: raw.system_component,
        parent_key_name: raw.parent_key_name,
    })
    .ok()?;
    let evidence_sha256 = format!("{:x}", Sha256::digest(evidence_json.as_bytes()));
    Some(ObservedInstallation {
        application_id,
        installation_id,
        source_key,
        scope: raw.scope,
        name,
        publisher,
        display_version,
        install_location,
        evidence_json,
        evidence_sha256,
    })
}

#[cfg(windows)]
fn registry_scan() -> Result<Vec<ObservedInstallation>, String> {
    use std::io::ErrorKind;
    use winreg::{
        RegKey,
        enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        },
    };
    let path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    let sources = [
        ("HKLM64", HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY, "SYSTEM"),
        ("HKLM32", HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY, "SYSTEM"),
        ("HKCU64", HKEY_CURRENT_USER, KEY_WOW64_64KEY, "USER"),
        ("HKCU32", HKEY_CURRENT_USER, KEY_WOW64_32KEY, "USER"),
    ];
    let mut found = Vec::new();
    for (label, hive, view, scope) in sources {
        let root = match RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ | view) {
            Ok(root) => root,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!(
                    "Cannot read {label} uninstall registrations: {error}"
                ));
            }
        };
        for entry in root.enum_keys() {
            let subkey = entry
                .map_err(|e| format!("Cannot enumerate {label} uninstall registrations: {e}"))?;
            let key = root
                .open_subkey_with_flags(&subkey, KEY_READ | view)
                .map_err(|e| format!("Cannot inspect {label} uninstall registration: {e}"))?;
            let read = |name: &str| key.get_value::<String, _>(name).ok();
            let raw = RawRegistration {
                source_key: format!("{label}\\{path}\\{subkey}"),
                scope: scope.into(),
                display_name: read("DisplayName"),
                publisher: read("Publisher"),
                display_version: read("DisplayVersion"),
                install_location: read("InstallLocation"),
                windows_installer: key.get_value("WindowsInstaller").ok(),
                system_component: key.get_value("SystemComponent").ok(),
                parent_key_name: read("ParentKeyName"),
            };
            if let Some(item) = normalize(raw) {
                found.push(item);
            }
            if found.len() > 20_000 {
                return Err("Installed application inventory exceeds the supported limit".into());
            }
        }
    }
    Ok(found)
}

impl InstalledInventorySource for LocalStorage {
    fn scan_installed(&self) -> Result<Vec<ObservedInstallation>, String> {
        #[cfg(windows)]
        {
            registry_scan()
        }
        #[cfg(not(windows))]
        {
            Err("Windows installed-application inventory is available on Windows only".into())
        }
    }
}

pub(crate) fn upsert(
    tx: &rusqlite::Transaction<'_>,
    item: &ObservedInstallation,
    now: i64,
) -> Result<(), String> {
    tx.execute("INSERT INTO applications(id,canonical_name,publisher,created_at_ms) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO NOTHING",
        params![item.application_id,item.name,item.publisher,now]).map_err(|e|e.to_string())?;
    tx.execute("INSERT INTO windows_installations(id,application_id,source_key,scope,display_name,publisher,display_version,install_location,evidence_json,evidence_sha256,state,first_seen_at_ms,last_seen_at_ms,last_verified_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'PRESENT',?11,?11,?11)
        ON CONFLICT(id) DO UPDATE SET application_id=excluded.application_id,display_name=excluded.display_name,publisher=excluded.publisher,display_version=excluded.display_version,install_location=excluded.install_location,evidence_json=excluded.evidence_json,evidence_sha256=excluded.evidence_sha256,state=CASE WHEN windows_installations.evidence_sha256=excluded.evidence_sha256 THEN 'PRESENT' ELSE 'CHANGED' END,last_seen_at_ms=excluded.last_seen_at_ms,last_verified_at_ms=excluded.last_verified_at_ms,revision=windows_installations.revision+1",
        params![item.installation_id,item.application_id,item.source_key,item.scope,item.name,item.publisher,item.display_version,item.install_location,item.evidence_json,item.evidence_sha256,now]).map_err(|e|e.to_string())?;
    Ok(())
}

impl InstalledInventoryRepository for SqliteCasks {
    fn reconcile_installed(
        &self,
        observed: &[ObservedInstallation],
        now: i64,
    ) -> Result<usize, String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS observed_windows_ids(id TEXT PRIMARY KEY); DELETE FROM observed_windows_ids;").map_err(|e|e.to_string())?;
        for item in observed {
            upsert(&tx, item, now)?;
            tx.execute(
                "INSERT INTO observed_windows_ids(id) VALUES (?1)",
                [&item.installation_id],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.execute("UPDATE windows_installations SET state='MISSING',last_verified_at_ms=?1,revision=revision+1 WHERE id NOT IN (SELECT id FROM observed_windows_ids) AND state!='MISSING'",[now]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(observed.len())
    }

    fn associate_installed(
        &self,
        cask_id: &str,
        item: &ObservedInstallation,
        now: i64,
    ) -> Result<DetectedApplicationView, String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        let state: String = tx
            .query_row("SELECT state FROM casks WHERE id=?1", [cask_id], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if state != "ACTIVE" {
            return Err("Cask is not active".into());
        }
        let msi_busy: bool = tx
            .query_row("SELECT EXISTS(SELECT 1 FROM msi_cleanup_locks)", [], |r| {
                r.get(0)
            })
            .map_err(|e| e.to_string())?;
        if msi_busy {
            return Err("Application cleanup is active".into());
        }
        let uninstalling:i64=tx.query_row("SELECT COUNT(*) FROM winget_installations w JOIN winget_locks l ON l.cask_id=w.cask_id JOIN winget_operations o ON o.id=l.operation_id AND o.action='UNINSTALL' WHERE w.registry_installation_id=?1 AND w.state='INSTALLED'",[&item.installation_id],|r|r.get(0)).map_err(|e|e.to_string())?;
        if uninstalling != 0 {
            return Err("Installation has an active uninstall operation".into());
        }
        upsert(&tx, item, now)?;
        tx.execute("INSERT INTO windows_cask_associations(id,cask_id,installation_id,created_at_ms) VALUES (?1,?2,?3,?4) ON CONFLICT DO NOTHING",
            params![Uuid::new_v4().to_string(),cask_id,item.installation_id,now]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![cask_id,now]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(observed_view(item, false, Some(cask_id.into())))
    }

    fn disassociate_installed(
        &self,
        cask_id: &str,
        installation_id: &str,
        now: i64,
    ) -> Result<(), String> {
        let mut connection = self.connection()?;
        let tx = connection.transaction().map_err(|e| e.to_string())?;
        let blocked: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_msi_installations WHERE installation_id=?1 AND cask_id=?2 AND state='OWNED') OR EXISTS(SELECT 1 FROM msi_cleanup_locks WHERE cask_id=?2)", params![installation_id,cask_id], |r|r.get(0)).map_err(|e|e.to_string())?;
        if blocked {
            return Err("Use managed application cleanup or explicitly keep the installation before detaching it".into());
        }
        let changed = tx.execute("UPDATE windows_cask_associations SET removed_at_ms=?3,revision=revision+1 WHERE cask_id=?1 AND installation_id=?2 AND removed_at_ms IS NULL",params![cask_id,installation_id,now]).map_err(|e|e.to_string())?;
        if changed != 1 {
            return Err("Active association not found".into());
        }
        tx.execute("UPDATE installer_operations SET installed_for_cask=0 WHERE cask_id=?1 AND confirmed_installation_id=?2",params![cask_id,installation_id]).map_err(|e|e.to_string())?;
        tx.execute("INSERT INTO projection_jobs(cask_id,target_revision,state,updated_at_ms) VALUES (?1,1,'PENDING',?2) ON CONFLICT(cask_id) DO UPDATE SET target_revision=target_revision+1,state='PENDING',updated_at_ms=excluded.updated_at_ms",params![cask_id,now]).map_err(|e|e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn list_associated_installed(
        &self,
        cask_id: &str,
    ) -> Result<Vec<DetectedApplicationView>, String> {
        let connection = self.connection()?;
        let mut stmt = connection.prepare("SELECT w.application_id,w.id,w.display_name,w.publisher,w.display_version,w.scope,w.source_key,w.install_location,w.state FROM windows_installations w JOIN windows_cask_associations a ON a.installation_id=w.id WHERE a.cask_id=?1 AND a.removed_at_ms IS NULL ORDER BY w.display_name COLLATE NOCASE,w.id").map_err(|e|e.to_string())?;
        stmt.query_map([cask_id], |row| {
            Ok(DetectedApplicationView {
                version: CONTRACT_VERSION,
                application_id: row.get(0)?,
                installation_id: row.get(1)?,
                name: row.get(2)?,
                publisher: row.get(3)?,
                display_version: row.get(4)?,
                scope: row.get(5)?,
                source_key: row.get(6)?,
                install_location: row.get(7)?,
                state: row.get(8)?,
                ambiguous_name: false,
                associated_cask_id: Some(cask_id.into()),
                provenance: if connection.query_row("SELECT EXISTS(SELECT 1 FROM installer_operations WHERE cask_id=?1 AND confirmed_installation_id=?2 AND installed_for_cask=1)",params![cask_id,row.get::<_,String>(1)?],|r|r.get::<_,bool>(0))? { "INSTALLED_BY_CASKLET" } else { "DETECTED_WINDOWS_REGISTRY" }.into(),
                removal_policy: if connection.query_row("SELECT EXISTS(SELECT 1 FROM managed_msi_installations WHERE installation_id=?1 AND cask_id=?2 AND state='OWNED')",params![row.get::<_,String>(1)?,cask_id],|r|r.get::<_,bool>(0))? { "MANAGED_MSI" } else { "DETACH_ONLY" }.into(),
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use casklet_application::CaskService;
    use casklet_contracts::CreateCask;
    fn raw(key: &str, name: Option<&str>) -> RawRegistration {
        RawRegistration {
            source_key: key.into(),
            scope: "USER".into(),
            display_name: name.map(str::to_owned),
            publisher: Some("Vendor".into()),
            display_version: None,
            install_location: None,
            windows_installer: None,
            system_component: None,
            parent_key_name: None,
        }
    }
    #[test]
    fn malformed_records_are_skipped_and_equal_names_remain_distinct() {
        assert!(normalize(raw("one", None)).is_none());
        assert!(normalize(raw("two", Some("\u{0000}bad"))).is_none());
        let first = normalize(raw("HKCU64\\One", Some("Editor"))).unwrap();
        let second = normalize(raw("HKCU32\\Two", Some("Editor"))).unwrap();
        assert_ne!(first.application_id, second.application_id);
        assert_ne!(first.installation_id, second.installation_id);
    }
    #[test]
    fn refresh_keeps_missing_history_and_detach_never_mutates_installation() {
        let path =
            std::env::temp_dir().join(format!("casklet-win-inventory-{}.db", Uuid::new_v4()));
        let repo = SqliteCasks::open(path).unwrap();
        let item = normalize(raw("HKCU64\\Example", Some("Example"))).unwrap();
        repo.reconcile_installed(std::slice::from_ref(&item), 1)
            .unwrap();
        let cask_id = Uuid::new_v4().to_string();
        repo.connection().unwrap().execute("INSERT INTO casks(id,name,state,managed_storage_root,storage_key,created_at_ms,updated_at_ms,revision) VALUES (?1,'Test','ACTIVE','test',?1,1,1,0)",[&cask_id]).unwrap();
        let associated = repo.associate_installed(&cask_id, &item, 2).unwrap();
        assert_eq!(associated.removal_policy, "DETACH_ONLY");
        let mut changed = item.clone();
        changed.display_version = Some("2.0".into());
        changed.evidence_json = format!("{}-version-2", item.evidence_json);
        changed.evidence_sha256 = format!("{:x}", Sha256::digest(changed.evidence_json.as_bytes()));
        repo.reconcile_installed(&[changed], 3).unwrap();
        assert_eq!(
            repo.list_associated_installed(&cask_id).unwrap()[0].state,
            "CHANGED"
        );
        repo.reconcile_installed(&[], 3).unwrap();
        assert_eq!(
            repo.list_associated_installed(&cask_id).unwrap()[0].state,
            "MISSING"
        );
        repo.disassociate_installed(&cask_id, &item.installation_id, 4)
            .unwrap();
        assert!(repo.list_associated_installed(&cask_id).unwrap().is_empty());
        let state: String = repo
            .connection()
            .unwrap()
            .query_row(
                "SELECT state FROM windows_installations WHERE id=?1",
                [&item.installation_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(state, "MISSING");
    }
    #[test]
    fn manifest_recovery_preserves_association_without_claiming_verification() {
        let root = std::env::temp_dir().join(format!("casklet-win-recovery-{}", Uuid::new_v4()));
        let storage = LocalStorage::new(root.join("casks"));
        let service = CaskService {
            repository: SqliteCasks::open(root.join("source.db")).unwrap(),
            storage: storage.clone(),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Inventory".into(),
                },
                "test",
            )
            .unwrap();
        let item = normalize(raw("HKCU64\\Recovered", Some("Recovered"))).unwrap();
        service
            .repository
            .associate_installed(&cask.id, &item, 1)
            .unwrap();
        service.retry_manifest_projections("test").unwrap();
        let manifest = root
            .join("casks")
            .join(&cask.id)
            .join(".casklet")
            .join("manifest.json");
        let target = SqliteCasks::open(root.join("target.db")).unwrap();
        target.recover_manifest(&storage, &manifest).unwrap();
        let associated = target.list_associated_installed(&cask.id).unwrap();
        assert_eq!(associated.len(), 1);
        assert_eq!(associated[0].state, "AMBIGUOUS");
        assert_eq!(associated[0].removal_policy, "DETACH_ONLY");
        assert_eq!(associated[0].installation_id, item.installation_id);
    }
}
