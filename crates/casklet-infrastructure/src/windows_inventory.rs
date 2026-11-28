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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    uninstall_string: Option<String>,
    windows_installer: Option<u32>,
    system_component: Option<u32>,
    parent_key_name: Option<String>,
}

fn trimmed(value: Option<String>, max: usize) -> Option<String> {
    value
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && v.chars().count() <= max && !v.chars().any(char::is_control))
}

fn executable_path(path: &str) -> Result<Option<std::path::PathBuf>, String> {
    let path = std::path::Path::new(path);
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Ok(None);
    }
    match path.canonicalize() {
        Ok(path) => Ok(path.is_file().then_some(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("Cannot check application executable: {error}")),
    }
}

fn broad_install_location(root: &std::path::Path) -> bool {
    if root.parent().is_none() {
        return true;
    }
    let mut protected = [
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "ProgramFiles",
        "ProgramFiles(x86)",
        "ProgramData",
        "SystemRoot",
    ]
    .iter()
    .filter_map(std::env::var_os)
    .map(std::path::PathBuf::from)
    .collect::<Vec<_>>();
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        for child in ["Downloads", "Desktop", "Documents"] {
            protected.push(std::path::PathBuf::from(&profile).join(child));
        }
    }
    protected
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .any(|path| path == root)
}

/// Location is only a reason to refuse installer execution, never ownership evidence.
pub(super) fn executable_in_inventory(
    path: &str,
    inventory: &[ObservedInstallation],
) -> Result<Option<String>, String> {
    let Some(executable) = executable_path(path)? else {
        return Ok(None);
    };
    for item in inventory {
        if let Some(location) = item.install_location.as_deref()
            && std::path::Path::new(location).is_absolute()
            && let Ok(root) = std::path::Path::new(location).canonicalize()
            && !broad_install_location(&root)
            && executable.starts_with(&root)
        {
            return Ok(Some(item.name.clone()));
        }
    }
    Ok(None)
}

/// Inspects current registration paths only; does not inspect or launch executable contents.
pub(super) fn installed_executable(path: &str) -> Result<Option<String>, String> {
    if executable_path(path)?.is_none() {
        return Ok(None);
    }
    #[cfg(windows)]
    {
        let inventory = registry_scan()?;
        if let Some(name) = executable_in_inventory(path, &inventory)? {
            return Ok(Some(name));
        }
        registered_executable_path(path, &inventory)
    }
    #[cfg(not(windows))]
    {
        Ok(None)
    }
}

#[cfg(windows)]
fn registered_executable_path(
    path: &str,
    inventory: &[ObservedInstallation],
) -> Result<Option<String>, String> {
    use winreg::{
        RegKey,
        enums::{
            HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
        },
    };
    let Some(executable) = executable_path(path)? else {
        return Ok(None);
    };
    let same_executable = |value: &str| {
        let value = value.trim();
        let file = if let Some(quoted) = value.strip_prefix('"') {
            quoted.split_once('"').map_or(quoted, |(path, _)| path)
        } else {
            value
                .rsplit_once(',')
                .filter(|(_, index)| index.trim().parse::<i32>().is_ok())
                .map_or(value, |(path, _)| path)
        };
        executable_path(file.trim())
            .ok()
            .flatten()
            .is_some_and(|candidate| {
                candidate == executable
                    || same_file::is_same_file(&candidate, &executable).unwrap_or(false)
            })
    };
    for item in inventory {
        let Some((view, path)) = item.source_key.split_once('\\') else {
            continue;
        };
        let (hive, flags) = match view {
            "HKCU64" => (HKEY_CURRENT_USER, KEY_WOW64_64KEY),
            "HKCU32" => (HKEY_CURRENT_USER, KEY_WOW64_32KEY),
            "HKLM64" => (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            "HKLM32" => (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
            _ => continue,
        };
        if let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(path, KEY_READ | flags)
            && let Ok(icon) = key.get_value::<String, _>("DisplayIcon")
            && same_executable(&icon)
        {
            return Ok(Some(item.name.clone()));
        }
    }
    // App Paths covers launchable applications whose uninstall record omits InstallLocation.
    if let Some(file) = executable.file_name() {
        for (hive, view) in [
            (HKEY_CURRENT_USER, KEY_WOW64_64KEY),
            (HKEY_CURRENT_USER, KEY_WOW64_32KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
        ] {
            let key_path = format!(
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{}",
                file.to_string_lossy()
            );
            if let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(key_path, KEY_READ | view)
                && let Ok(value) = key.get_value::<String, _>("")
                && same_executable(&value)
            {
                return Ok(Some(file.to_string_lossy().into_owned()));
            }
        }
    }
    Ok(None)
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
        // MSI uses typed ProductCode uninstall; preserve pre-schema-16 evidence hashes.
        uninstall_string: if raw.windows_installer == Some(1) {
            None
        } else {
            raw.uninstall_string
        },
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
                uninstall_string: read("UninstallString"),
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

/// Size metadata is deliberately outside the immutable uninstall evidence hash.
pub(super) fn registration_size(source_key: &str) -> Option<u64> {
    #[cfg(windows)]
    {
        use winreg::{
            RegKey,
            enums::{
                HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
            },
        };
        let (view, path) = source_key.split_once('\\')?;
        let (hive, flags) = match view {
            "HKCU64" => (HKEY_CURRENT_USER, KEY_WOW64_64KEY),
            "HKCU32" => (HKEY_CURRENT_USER, KEY_WOW64_32KEY),
            "HKLM64" => (HKEY_LOCAL_MACHINE, KEY_WOW64_64KEY),
            "HKLM32" => (HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY),
            _ => return None,
        };
        let key = RegKey::predef(hive)
            .open_subkey_with_flags(path, KEY_READ | flags)
            .ok()?;
        Some(u64::from(key.get_value::<u32, _>("EstimatedSize").ok()?).saturating_mul(1024))
    }
    #[cfg(not(windows))]
    {
        let _ = source_key;
        None
    }
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
    // Cheap registry metadata captured during explicit inventory/association; never a tree scan.
    let estimate = registration_size(&item.source_key).and_then(|value| i64::try_from(value).ok());
    tx.execute("INSERT INTO application_size_snapshots(source_key,evidence_sha256,size_bytes,estimated,measured_at_ms) VALUES (?1,?2,?3,1,?4) ON CONFLICT(source_key) DO UPDATE SET evidence_sha256=excluded.evidence_sha256,size_bytes=excluded.size_bytes,estimated=1,measured_at_ms=excluded.measured_at_ms WHERE application_size_snapshots.evidence_sha256!=excluded.evidence_sha256 OR application_size_snapshots.size_bytes IS NULL", params![item.source_key,item.evidence_sha256,estimate,now]).map_err(|e|e.to_string())?;
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
                removal_policy: connection.query_row("SELECT COALESCE((SELECT CASE WHEN product_code LIKE '{%}' THEN 'MANAGED_MSI' ELSE 'CASKLET_OWNED' END FROM managed_msi_installations WHERE installation_id=?1 AND cask_id=?2 AND state='OWNED'),'DETACH_ONLY')",params![row.get::<_,String>(1)?,cask_id],|r|r.get::<_,String>(0))?,
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
    #[test]
    fn registry_aliases_require_shared_user_keys_and_equal_evidence() {
        let path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Example";
        let left = normalize(raw(&format!("HKCU64\\{path}"), Some("Example"))).unwrap();
        let right = normalize(raw(&format!("HKCU32\\{path}"), Some("Example"))).unwrap();
        assert!(left.is_user_registration_alias_of(&right));
        let mut changed = right.clone();
        changed.evidence_json = "{}".into();
        assert!(!left.is_user_registration_alias_of(&changed));
        let mut changed = right.clone();
        changed.source_key.push_str("Other");
        assert!(!left.is_user_registration_alias_of(&changed));
        let machine_left = normalize(raw(&format!("HKLM64\\{path}"), Some("Example"))).unwrap();
        let machine_right = normalize(raw(&format!("HKLM32\\{path}"), Some("Example"))).unwrap();
        assert!(!machine_left.is_user_registration_alias_of(&machine_right));
    }

    #[cfg(windows)]
    #[test]
    fn installed_executable_guard_checks_registered_location_and_icon_without_execution() {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        struct Registration(String);
        impl Drop for Registration {
            fn drop(&mut self) {
                let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey(&self.0);
            }
        }
        let name = format!("Casklet-executable-guard-{}", Uuid::new_v4());
        let registration = Registration(format!(
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{name}"
        ));
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey(&registration.0)
            .unwrap();
        key.set_value("DisplayName", &name).unwrap();
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join(&name);
        let installed = root.join("installed");
        std::fs::create_dir_all(&installed).unwrap();
        let exe = installed.join("application.exe");
        std::fs::write(
            &exe,
            b"not executable; detection must not execute or inspect PE bytes",
        )
        .unwrap();
        key.set_value("InstallLocation", &installed.to_string_lossy().as_ref())
            .unwrap();
        assert_eq!(
            installed_executable(&exe.to_string_lossy()).unwrap(),
            Some(name.clone())
        );
        key.delete_value("InstallLocation").unwrap();
        key.set_value("DisplayIcon", &format!("\"{}\",0", exe.display()))
            .unwrap();
        assert_eq!(
            installed_executable(&exe.to_string_lossy()).unwrap(),
            Some(name.clone())
        );
        let setup = root.join("setup.exe");
        std::fs::write(&setup, b"separate setup").unwrap();
        assert_eq!(
            installed_executable(&setup.to_string_lossy()).unwrap(),
            None
        );
        let alias = root.join("same-application.exe");
        std::fs::hard_link(&exe, &alias).unwrap();
        assert_eq!(
            installed_executable(&alias.to_string_lossy()).unwrap(),
            Some(name)
        );
    }
    #[cfg(windows)]
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "native lifecycle fixture keeps its registry cleanup guard in one scope"
    )]
    fn native_shared_user_views_identify_the_same_fixture_registration() {
        use casklet_contracts::GetStorageSummary;
        use winreg::{
            RegKey,
            enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE},
        };
        struct FixtureKey {
            root: RegKey,
            name: String,
        }
        impl Drop for FixtureKey {
            fn drop(&mut self) {
                let _ = self.root.delete_subkey(&self.name);
            }
        }
        let path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
        let root = RegKey::predef(HKEY_CURRENT_USER)
            .create_subkey_with_flags(path, KEY_READ | KEY_WRITE)
            .unwrap()
            .0;
        let name = format!("Casklet.RegistryAlias.Test.{}", Uuid::new_v4());
        assert!(root.open_subkey(&name).is_err());
        let (key, _) = root.create_subkey(&name).unwrap();
        let fixture = FixtureKey {
            root,
            name: name.clone(),
        };
        key.set_value("DisplayName", &"Casklet registry-alias fixture")
            .unwrap();
        key.set_value("Publisher", &"Casklet test").unwrap();
        key.set_value("EstimatedSize", &12_u32).unwrap();
        let test_root = std::env::temp_dir().join(format!("casklet-storage-{}", Uuid::new_v4()));
        let service = CaskService {
            repository: SqliteCasks::open(test_root.join("state.db")).unwrap(),
            storage: LocalStorage::new(test_root.join("casks")),
        };
        let cask = service
            .create(
                &CreateCask {
                    version: CONTRACT_VERSION,
                    name: "Storage".into(),
                },
                "test",
            )
            .unwrap();
        let inventory = registry_scan().unwrap();
        let entries: Vec<_> = inventory
            .iter()
            .filter(|item| item.source_key.ends_with(&format!("\\{name}")))
            .collect();
        assert_eq!(entries.len(), 2);
        assert_ne!(entries[0].installation_id, entries[1].installation_id);
        assert!(entries[0].is_user_registration_alias_of(entries[1]));
        assert!(entries[1].is_user_registration_alias_of(entries[0]));
        let request = GetStorageSummary {
            version: CONTRACT_VERSION,
            cask_id: cask.id.clone(),
        };
        let summary = || service.storage_summary(&request, "test").unwrap();
        assert_eq!(summary().reference_application_bytes, 0);
        service
            .repository
            .associate_installed(&cask.id, entries[0], 1)
            .unwrap();
        service
            .repository
            .associate_installed(&cask.id, entries[1], 1)
            .unwrap();
        assert_eq!(summary().reference_application_bytes, 12 * 1024); // shared views counted once
        assert_eq!(summary().estimated_applications, 1);
        // Persistent ownership changes classification; a failed uninstall keeps these bytes.
        let connection = service.repository.connection().unwrap();
        let resource = Uuid::new_v4().to_string();
        let artifact = Uuid::new_v4().to_string();
        let operation = Uuid::new_v4().to_string();
        connection.execute("INSERT INTO resources(id,kind,display_name,management,provenance,availability,size_bytes,size_measurement,unknown_entries,created_at_ms) VALUES (?1,'FILE','fixture','MANAGED','COPIED_BY_CASKLET','PRESENT',9000,'EXACT',0,1)", [&resource]).unwrap();
        connection.execute("INSERT INTO installer_artifacts(id,cask_id,resource_id,source_path,path,format,sha256,size_bytes,inspection_json,registered_at_ms) VALUES (?1,?2,?3,'fixture','fixture','EXE','fixture',9000,'{}',1)", params![artifact,cask.id,resource]).unwrap();
        connection.execute("INSERT INTO installer_operations(id,artifact_id,cask_id,state,started_at_ms,updated_at_ms) VALUES (?1,?2,?3,'CONFIRMED_ASSOCIATION',1,1)", params![operation,artifact,cask.id]).unwrap();
        let installer_path = test_root.join("installer.exe");
        std::fs::write(&installer_path, vec![0_u8; 9000]).unwrap();
        connection.execute("INSERT INTO resource_locations(id,resource_id,path_display,path_key,containment,is_primary,first_seen_at_ms) VALUES (?1,?2,?3,?3,'INTERNAL',1,1)", params![Uuid::new_v4().to_string(),resource,installer_path.to_string_lossy()]).unwrap();
        connection.execute("INSERT INTO resource_associations(id,cask_id,resource_id,category_key,relationship,removal_policy,added_at_ms) VALUES (?1,?2,?3,'applications','OWNED','DELETE_WHEN_UNREFERENCED',1)", params![Uuid::new_v4().to_string(),cask.id,resource]).unwrap();
        assert_eq!(summary().managed_logical_bytes, 0); // retained installer copy excluded
        assert_eq!(summary().owned_application_bytes, 0); // artifact/operation alone grants no installed bytes
        connection.execute("INSERT INTO managed_msi_installations(installation_id,cask_id,operation_id,product_code,evidence_sha256,display_name,state,claimed_at_ms) VALUES (?1,?2,?3,'fixture',?4,'fixture','OWNED',1)", params![entries[0].installation_id,cask.id,operation,entries[0].evidence_sha256]).unwrap();
        assert_eq!(summary().owned_application_bytes, 12 * 1024);
        assert_eq!(summary().reference_application_bytes, 0);
        connection
            .execute(
                "UPDATE managed_msi_installations SET state='REMOVED' WHERE installation_id=?1",
                [&entries[0].installation_id],
            )
            .unwrap();
        assert_eq!(summary().owned_application_bytes, 0);
        drop(connection);
        let evidence = registry_scan()
            .unwrap()
            .into_iter()
            .find(|item| item.installation_id == entries[0].installation_id)
            .unwrap()
            .evidence_sha256;
        key.set_value("EstimatedSize", &24_u32).unwrap();
        assert_eq!(summary().reference_application_bytes, 24 * 1024);
        assert_eq!(
            registry_scan()
                .unwrap()
                .into_iter()
                .find(|item| item.installation_id == entries[0].installation_id)
                .unwrap()
                .evidence_sha256,
            evidence
        );
        service
            .repository
            .disassociate_installed(&cask.id, &entries[0].installation_id, 2)
            .unwrap();
        service
            .repository
            .disassociate_installed(&cask.id, &entries[1].installation_id, 2)
            .unwrap();
        assert_eq!(summary().reference_application_bytes, 0);
        assert!(fixture.root.open_subkey(&name).is_ok()); // detach never uninstalls
        service
            .repository
            .associate_installed(&cask.id, entries[0], 3)
            .unwrap();
        let installation_folder = test_root.join("installed-app");
        std::fs::create_dir_all(&installation_folder).unwrap();
        let payload = installation_folder.join("app.bin");
        std::fs::write(&payload, b"1234567").unwrap();
        key.set_value(
            "InstallLocation",
            &installation_folder.to_string_lossy().as_ref(),
        )
        .unwrap();
        assert_eq!(summary().reference_application_bytes, 7);
        assert_eq!(summary().estimated_applications, 0);
        std::fs::write(&payload, b"12345678901").unwrap();
        assert_eq!(summary().reference_application_bytes, 11);
        drop(key);
        drop(fixture);
        assert_eq!(summary().reference_application_bytes, 0); // actual absence overrides history
        assert_eq!(
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(format!("{path}\\{name}"))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::NotFound
        );
    }
    fn raw(key: &str, name: Option<&str>) -> RawRegistration {
        RawRegistration {
            source_key: key.into(),
            scope: "USER".into(),
            display_name: name.map(str::to_owned),
            publisher: Some("Vendor".into()),
            display_version: None,
            install_location: None,
            uninstall_string: None,
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
