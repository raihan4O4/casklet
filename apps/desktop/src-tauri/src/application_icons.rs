//! Optional desktop presentation data. Never participates in inventory or ownership.
use serde::Deserialize;
#[cfg(windows)]
use tauri::Manager;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IconSource {
    Registry { key: String },
    Portable { id: String },
}

#[tauri::command]
pub async fn application_icons(
    app: tauri::AppHandle,
    service: tauri::State<'_, crate::Service>,
    sources: Vec<IconSource>,
) -> Result<Vec<Option<String>>, String> {
    if sources.len() > 16 {
        return Err("At most 16 application icons per request".into());
    }
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let paths = sources
            .into_iter()
            .map(|source| match source {
                IconSource::Registry { key } => {
                    if valid_registry_key(&key) {
                        format!("registry:{key}")
                    } else {
                        String::new()
                    }
                }
                IconSource::Portable { id } => service
                    .get_portable_installation(
                        &casklet_contracts::GetPortableInstallation {
                            version: casklet_contracts::CONTRACT_VERSION,
                            installation_id: id,
                        },
                        "desktop-icon",
                    )
                    .map(|item| {
                        std::path::Path::new(&item.root_path)
                            .join(item.executable_relative_path)
                            .to_string_lossy()
                            .into_owned()
                    })
                    .unwrap_or_default(),
            })
            .collect::<Vec<_>>();
        load(&app, &paths)
    })
    .await
    .map_err(|error| error.to_string())?
}

fn valid_registry_key(key: &str) -> bool {
    ["HKCU64", "HKCU32", "HKLM64", "HKLM32"].iter().any(|hive| {
        let prefix = format!("{hive}\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\");
        key.to_ascii_lowercase()
            .starts_with(&prefix.to_ascii_lowercase())
            && key.len() > prefix.len()
            && key.len() <= 4096
            && !key.contains('\0')
    })
}

#[cfg(windows)]
fn load(app: &tauri::AppHandle, paths: &[String]) -> Result<Vec<Option<String>>, String> {
    use std::{
        io::Read,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let helper = if cfg!(debug_assertions) {
        std::path::PathBuf::from(env!("CASKLET_ICON_HELPER"))
    } else {
        app.path()
            .resource_dir()
            .map_err(|error| error.to_string())?
            .join("application-icons/casklet-icons.exe")
    };
    let mut child = Command::new(helper)
        .args(paths)
        .creation_flags(0x0800_0000)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().ok_or("Icon output unavailable")?;
    let reader = std::thread::spawn(move || {
        let mut output = String::new();
        stdout
            .take(1_500_000)
            .read_to_string(&mut output)
            .map(|_| output)
    });
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            break;
        }
        if started.elapsed() > Duration::from_secs(4) {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err("Icon extraction timed out".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let output = reader
        .join()
        .map_err(|_| "Icon reader failed")?
        .map_err(|error| error.to_string())?;
    let mut lines = output.lines();
    Ok(paths
        .iter()
        .map(|_| {
            lines
                .next()
                .filter(|line| {
                    line.starts_with("iVBORw0KGgo")
                        && line.len() <= 90_000
                        && line
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b"+/=".contains(&byte))
                })
                .map(|line| format!("data:image/png;base64,{line}"))
        })
        .collect())
}

#[cfg(not(windows))]
fn load(_app: &tauri::AppHandle, paths: &[String]) -> Result<Vec<Option<String>>, String> {
    Ok(vec![None; paths.len()])
}

#[cfg(test)]
mod tests {
    use super::valid_registry_key;
    #[cfg(windows)]
    #[test]
    fn native_icon_fallbacks_are_bounded_and_ignore_ambiguous_sources() {
        let output = std::process::Command::new(env!("CASKLET_ICON_TESTS"))
            .output()
            .expect("Icon fallback regression executable");
        assert!(output.status.success(), "{output:?}");
    }
    #[test]
    fn only_uninstall_registration_keys_are_accepted() {
        assert!(valid_registry_key(
            "HKLM64\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Chrome"
        ));
        assert!(valid_registry_key(
            "HKLM32\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Chrome"
        ));
        for key in [
            "HKCU64\\Software\\Private",
            "C:\\app.exe",
            "registry:HKLM64",
            "HKLM64\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\",
            "HKLM64\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\bad\0",
        ] {
            assert!(!valid_registry_key(key));
        }
    }

    #[cfg(windows)]
    #[test]
    fn native_helper_returns_png_and_preserves_missing_source_positions() {
        let system_root = std::env::var_os("SystemRoot").expect("Windows directory");
        let executable = std::path::PathBuf::from(system_root).join("System32/notepad.exe");
        let output = std::process::Command::new(env!("CASKLET_ICON_HELPER"))
            .arg(executable)
            .args(["", r"\\server\share\app.exe", "relative.exe"])
            .output()
            .expect("Icon helper");
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).expect("PNG base64 output");
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with("iVBORw0KGgo"));
        assert!(lines[1..].iter().all(|line| line.is_empty()));
    }
}
