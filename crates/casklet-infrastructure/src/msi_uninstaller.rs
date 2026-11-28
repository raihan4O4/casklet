//! Windows Installer process adapter: typed arguments, visible installer UI, no console.
use super::LocalStorage;
use casklet_application::managed_msi::{MsiUninstaller, product_code};

impl MsiUninstaller for LocalStorage {
    fn inspect_registered_uninstaller(
        &self,
        item: &casklet_application::windows_inventory::ObservedInstallation,
    ) -> Result<String, String> {
        let (path, _) = registered_command(&item.evidence_json)?;
        let (_, hash) = executable(&path)?;
        Ok(hash)
    }
    fn uninstall_registered(
        &self,
        item: &casklet_application::windows_inventory::ObservedInstallation,
        expected: &str,
    ) -> Result<i32, String> {
        let (path, args) = registered_command(&item.evidence_json)?;
        let (_held, hash) = executable(&path)?;
        if hash != expected {
            return Err("Uninstaller executable changed".into());
        }
        #[cfg(windows)]
        {
            use std::process::{Command, Stdio};
            use std::time::{Duration, Instant};
            // The fixed helper treats path/arguments as environment data. Start-Process
            // waits for descendants; RunAs keeps application elevation visible.
            let script = r"$ErrorActionPreference='Stop'; try { $params=@{FilePath=$env:CASKLET_UNINSTALL_PATH;Verb='RunAs';Wait=$true;PassThru=$true}; if($env:CASKLET_UNINSTALL_ARGS){$params.ArgumentList=$env:CASKLET_UNINSTALL_ARGS}; $p=Start-Process @params; exit $p.ExitCode } catch { if($_.Exception.NativeErrorCode -eq 1223){exit 1223}; exit 125 }";
            let mut command = Command::new(super::windows_process::system_binary(
                "WindowsPowerShell\\v1.0\\powershell.exe",
            )?);
            command
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("CASKLET_UNINSTALL_PATH", &path)
                .env("CASKLET_UNINSTALL_ARGS", args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut child = super::windows_process::hide_console(&mut command)
                .spawn()
                .map_err(|e| e.to_string())?;
            let start = Instant::now();
            loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    return status.code().ok_or("Uninstaller interrupted".into());
                }
                if start.elapsed() > Duration::from_secs(3600) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("Uninstaller timed out; inspect Windows before retrying".into());
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        #[cfg(not(windows))]
        {
            let _ = args;
            Err("Registered uninstall requires Windows".into())
        }
    }
    fn uninstall_msi(&self, code: &str) -> Result<i32, String> {
        let code = product_code(code).ok_or("Invalid MSI ProductCode")?;
        #[cfg(windows)]
        {
            use std::process::{Command, Stdio};
            use std::time::{Duration, Instant};
            let mut command = Command::new(super::windows_process::system_binary("msiexec.exe")?);
            command
                .args(["/x", &code, "/norestart"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut child = super::windows_process::hide_console(&mut command)
                .spawn()
                .map_err(|e| e.to_string())?;
            let start = Instant::now();
            loop {
                if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                    return status.code().ok_or("MSI uninstall interrupted".into());
                }
                if start.elapsed() > Duration::from_secs(3600) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(
                        "MSI uninstall timed out; inspect Windows before closing cleanup".into(),
                    );
                }
                std::thread::sleep(Duration::from_millis(200));
            }
        }
        #[cfg(not(windows))]
        {
            let _ = code;
            Err("MSI uninstall is available on Windows only".into())
        }
    }
}

// Never send a registry command to a shell or resolve an executable through PATH.
fn registered_command(evidence: &str) -> Result<(String, String), String> {
    let value: serde_json::Value = serde_json::from_str(evidence).map_err(|e| e.to_string())?;
    let command = value["uninstallString"]
        .as_str()
        .ok_or("Uninstall registration has no command")?
        .trim();
    if command.len() > 16_384 || command.chars().any(char::is_control) {
        return Err("Invalid uninstall command".into());
    }
    let (path, args) = if let Some(quoted) = command.strip_prefix('"') {
        let end = quoted.find('"').ok_or("Unclosed executable path")?;
        if !quoted[end + 1..].is_empty() && !quoted[end + 1..].starts_with(' ') {
            return Err("Invalid executable boundary".into());
        }
        (&quoted[..end], quoted[end + 1..].trim())
    } else {
        command.split_once(' ').unwrap_or((command, ""))
    };
    let bytes = path.as_bytes();
    if bytes.len() < 4
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || bytes[2] != b'\\'
        || !path.to_ascii_lowercase().ends_with(".exe")
        || path.contains(['%', '"'])
        || path[2..].contains(':')
        || path.split('\\').any(|part| part == "..")
    {
        return Err("Uninstaller must be an exact local absolute EXE path".into());
    }
    let name = path.rsplit('\\').next().unwrap_or("").to_ascii_lowercase();
    if [
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "wscript.exe",
        "cscript.exe",
        "mshta.exe",
        "rundll32.exe",
        "regsvr32.exe",
        "msiexec.exe",
        "python.exe",
        "python3.exe",
        "node.exe",
        "bash.exe",
        "sh.exe",
    ]
    .contains(&name.as_str())
    {
        return Err("Shell and interpreter uninstall commands are unsupported".into());
    }
    Ok((path.into(), args.into()))
}
fn executable(path: &str) -> Result<(Vec<std::fs::File>, String), String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let path = std::path::Path::new(path);
    super::resource_storage::check_components(path)?;
    let mut held = Vec::new();
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        for parent in path.ancestors().skip(1) {
            held.push(
                std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(3)
                    .custom_flags(0x0200_0000)
                    .open(parent)
                    .map_err(|e| e.to_string())?,
            );
        }
        super::resource_storage::check_components(path)?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Uninstaller is not a file".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    held.push(file);
    Ok((held, format!("{:x}", hash.finalize())))
}
#[cfg(test)]
mod registered_tests {
    use super::*;
    #[test]
    fn accepts_direct_executables_and_rejects_shells_and_ambiguous_paths() {
        let evidence = |s: &str| serde_json::json!({"uninstallString":s}).to_string();
        assert_eq!(
            registered_command(&evidence(r#""C:\Program Files\Example\uninstall.exe" /S"#))
                .unwrap(),
            (
                r"C:\Program Files\Example\uninstall.exe".into(),
                "/S".into()
            )
        );
        for command in [
            r"cmd.exe /c erase",
            r"C:\Windows\System32\cmd.exe /c erase",
            r"C:\Program Files\Example\uninstall.exe",
            r"\\host\app.exe",
            r"C:\x\..\app.exe",
            r#""C:\app.exe"evil"#,
            r"%TEMP%\app.exe",
        ] {
            assert!(registered_command(&evidence(command)).is_err(), "{command}");
        }
    }
}
