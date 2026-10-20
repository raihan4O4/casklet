//! Disposable Explorer menu projection. Registry data never grants resource authority.
#[cfg(windows)]
use {crate::SqliteCasks, casklet_application::CaskRepository, std::path::Path, winreg::RegKey};

#[cfg(windows)]
const SETTINGS: &str = "Software\\Casklet\\Explorer";
#[cfg(windows)]
const LEGACY_CLASS_ID: &str = "{960e5343-e03e-47cd-b1a3-d58632778824}";

#[cfg(windows)]
fn class_id_for_library(library: &Path) -> Result<String, String> {
    let name = library
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("Invalid Explorer adapter filename")?;
    let hash = name
        .strip_prefix("casklet-explorer-")
        .and_then(|name| name.rsplit('-').next())
        .filter(|value| value.len() == 16)
        .ok_or("Unversioned Explorer adapter")?;
    let hash = u64::from_str_radix(hash, 16).map_err(|_| "Invalid Explorer adapter identity")?;
    let class_hash = (hash & 0x3fff_ffff_ffff_ffff) | 0x8000_0000_0000_0000;
    Ok(format!(
        "{{960e5343-e03e-47cd-{:04x}-{:012x}}}",
        class_hash >> 48,
        class_hash & 0xffff_ffff_ffff
    ))
}

/// Registers the installed executable and refreshes the current user's menu.
/// # Errors
/// Returns registry, database, or installation-path errors.
#[cfg(windows)]
pub fn register(executable: &Path, library: &Path) -> Result<(), String> {
    let directory = executable
        .parent()
        .ok_or("Installed executable has no directory")?
        .join("explorer");
    if library.parent() != Some(directory.as_path()) || !library.is_file() {
        return Err("The packaged Explorer adapter is missing. Reinstall Casklet.".into());
    }
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    let db = Path::new(&local).join("CaskletData/db/casklet.db");
    let repository = SqliteCasks::open(db.clone())?;
    let settings = RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .create_subkey(SETTINGS)
        .map_err(|e| e.to_string())?
        .0;
    settings
        .set_value("Executable", &executable.as_os_str())
        .map_err(|e| e.to_string())?;
    settings
        .set_value("Database", &db.as_os_str())
        .map_err(|e| e.to_string())?;
    refresh(&repository)?;
    let user = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let classes = user
        .create_subkey("Software\\Classes")
        .map_err(|e| e.to_string())?
        .0;
    register_handler(&classes, library)?;
    settings
        .set_value("ClassId", &class_id_for_library(library)?)
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
pub(crate) fn refresh(repository: &SqliteCasks) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    let user = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(settings) = user.open_subkey_with_flags(SETTINGS, KEY_READ | KEY_WRITE) else {
        return Ok(());
    };
    let db: String = settings.get_value("Database").map_err(|e| e.to_string())?;
    if Path::new(&db) != repository.db_path {
        return Ok(());
    }
    let executable: String = settings
        .get_value("Executable")
        .map_err(|e| e.to_string())?;
    if !Path::new(&executable).is_file() || executable.contains('"') {
        return Err("Explorer executable is unavailable".into());
    }
    // Serialize projections across CLI/GUI processes, outside all database transactions.
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(repository.db_path.with_extension("explorer.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    publish(&settings, repository.list()?)
}

#[cfg(windows)]
fn publish(settings: &RegKey, mut casks: Vec<casklet_domain::Cask>) -> Result<(), String> {
    casks.retain(|cask| cask.state == casklet_domain::CaskState::Active);
    casks.sort_by(|a, b| (a.created_at_ms, &a.id).cmp(&(b.created_at_ms, &b.id)));
    let mut snapshot = Vec::<String>::new();
    for cask in casks.iter().take(5) {
        if cask.id.is_empty()
            || !cask
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            continue;
        }
        let mut title = cask.name.replace('&', "&&");
        if casks
            .iter()
            .filter(|other| other.name.to_lowercase() == cask.name.to_lowercase())
            .count()
            > 1
        {
            title.push_str(" · ");
            title.extend(cask.id.chars().take(8));
        }
        snapshot.push(title);
        snapshot.push(cask.id.clone());
    }
    if casks.len() > 5 || snapshot.is_empty() {
        snapshot.push(
            if snapshot.is_empty() {
                "Choose or create a cask…"
            } else {
                "More casks…"
            }
            .into(),
        );
        snapshot.push("choose".into());
    }
    // One atomic registry value binds every displayed name to its destination ID.
    settings
        .set_value("Snapshot", &snapshot)
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn register_handler(classes: &RegKey, library: &Path) -> Result<(), String> {
    let class_id = class_id_for_library(library)?;
    let previous = classes
        .open_subkey("*\\shellex\\ContextMenuHandlers\\Casklet")
        .ok()
        .and_then(|key| key.get_value::<String, _>("").ok());
    let server = classes
        .create_subkey(format!("CLSID\\{class_id}\\InprocServer32"))
        .map_err(|e| e.to_string())?
        .0;
    server
        .set_value("", &library.as_os_str())
        .map_err(|e| e.to_string())?;
    server
        .set_value("ThreadingModel", &"Apartment")
        .map_err(|e| e.to_string())?;
    for association in ["*", "Directory"] {
        let key = classes
            .create_subkey(format!(
                "{association}\\shellex\\ContextMenuHandlers\\Casklet"
            ))
            .map_err(|e| e.to_string())?
            .0;
        key.set_value("", &class_id).map_err(|e| e.to_string())?;
        // Retire only the earlier Casklet static projection, if present.
        match classes.delete_subkey_all(format!("{association}\\shell\\Casklet")) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    // Remove only retired Casklet registrations in our namespace that point
    // into this installation's adapter directory. Loaded DLLs remain untouched.
    for retired in previous.iter().map(String::as_str).chain([LEGACY_CLASS_ID]) {
        if retired == class_id
            || !retired
                .to_ascii_lowercase()
                .starts_with("{960e5343-e03e-47cd-")
        {
            continue;
        }
        let key = format!("CLSID\\{retired}");
        let path = classes
            .open_subkey(format!("{key}\\InprocServer32"))
            .ok()
            .and_then(|server| server.get_value::<String, _>("").ok());
        if path
            .as_deref()
            .is_some_and(|path| Path::new(path).parent() == library.parent())
        {
            classes.delete_subkey_all(key).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

impl crate::SqliteCasks {
    pub(crate) fn refresh_explorer(&self) {
        #[cfg(windows)]
        if let Err(problem) = refresh(self) {
            // A disposable menu projection must never turn a committed Cask mutation into failure.
            eprintln!("Explorer menu refresh failed: {problem}");
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    struct RegistryFixture<'a> {
        user: &'a RegKey,
        root: &'a str,
    }
    impl Drop for RegistryFixture<'_> {
        fn drop(&mut self) {
            let _ = self.user.delete_subkey_all(self.root);
        }
    }
    #[test]
    fn native_registry_projection_covers_limits_names_and_replacement() {
        // Isolated registry subtree: never register a real Explorer association.
        let user = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let root = format!("Software\\CaskletExplorerTests\\{}", uuid::Uuid::new_v4());
        let fixture = RegistryFixture {
            user: &user,
            root: &root,
        };
        let classes = user.create_subkey(format!("{root}\\Classes")).unwrap().0;
        let settings = user.create_subkey(format!("{root}\\Settings")).unwrap().0;
        let casks: Vec<_> = (0..8)
            .map(|index| {
                casklet_domain::Cask::new(
                    &format!("資料 & Cask {index}"),
                    "C:/fixture".into(),
                    index,
                )
                .unwrap()
            })
            .collect();
        for count in [0, 1, 5, 6, 8, 1] {
            publish(&settings, casks[..count].to_vec()).unwrap();
            let snapshot: Vec<String> = settings.get_value("Snapshot").unwrap();
            assert_eq!(
                snapshot.len(),
                2 * (count.min(5) + usize::from(count == 0 || count > 5))
            );
            if count > 0 {
                assert_eq!(snapshot[0], "資料 && Cask 0");
                assert_eq!(snapshot[1], casks[0].id);
            }
            if count == 0 || count > 5 {
                assert_eq!(snapshot.last().unwrap(), "choose");
            }
        }
        let mut duplicate = casks[1].clone();
        duplicate.name.clone_from(&casks[0].name);
        publish(&settings, vec![casks[0].clone(), duplicate.clone()]).unwrap();
        let snapshot: Vec<String> = settings.get_value("Snapshot").unwrap();
        assert_ne!(snapshot[0], snapshot[2]);
        let mut archived = casks[0].clone();
        archived.state = casklet_domain::CaskState::Archived;
        publish(&settings, vec![archived, duplicate.clone()]).unwrap();
        let snapshot: Vec<String> = settings.get_value("Snapshot").unwrap();
        assert_eq!(snapshot, [duplicate.name.replace('&', "&&"), duplicate.id]);
        let first = Path::new("C:/fixture/casklet-explorer-alpha-0123456789abcdef.dll");
        let second = Path::new("C:/fixture/casklet-explorer-alpha-fedcba9876543210.dll");
        let first_id = class_id_for_library(first).unwrap();
        let class_id = class_id_for_library(second).unwrap();
        assert_ne!(first_id, class_id);
        assert_eq!(first_id, "{960e5343-e03e-47cd-8123-456789abcdef}");
        assert!(class_id_for_library(Path::new("explorer.dll")).is_err());
        register_handler(&classes, first).unwrap();
        register_handler(&classes, second).unwrap();
        assert!(classes.open_subkey(format!("CLSID\\{first_id}")).is_err());
        let server = classes
            .open_subkey(format!("CLSID\\{class_id}\\InprocServer32"))
            .unwrap();
        assert_eq!(
            server.get_value::<String, _>("ThreadingModel").unwrap(),
            "Apartment"
        );
        drop(server);
        for association in ["*", "Directory"] {
            let handler = classes
                .open_subkey(format!(
                    "{association}\\shellex\\ContextMenuHandlers\\Casklet"
                ))
                .unwrap();
            assert_eq!(handler.get_value::<String, _>("").unwrap(), class_id);
        }
        drop((classes, settings));
        drop(fixture);
    }
}
