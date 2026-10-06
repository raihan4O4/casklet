//! Windows installer inspection and typed process launch. No installer is run during inspection.
use super::LocalStorage;
use super::windows_process::system_binary;
use casklet_application::installer::{InstallerOutcome, InstallerSource};
use casklet_contracts::{CONTRACT_VERSION, InstallerInspection};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const MAX_INSTALLER_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const MAX_METADATA_BYTES: usize = 64 * 1024;
const INSPECT_SCRIPT: &str = r#"
$ErrorActionPreference='Stop'
$p=$env:CASKLET_INSTALLER_PATH
$s=Get-AuthenticodeSignature -LiteralPath $p
$v=(Get-Item -LiteralPath $p).VersionInfo
$r=@{
  status=[string]$s.Status
  statusMessage=[string]$s.StatusMessage
  signer=if($s.SignerCertificate){[string]$s.SignerCertificate.Subject}else{$null}
  name=if($v.ProductName){[string]$v.ProductName}else{$null}
  publisher=if($v.CompanyName){[string]$v.CompanyName}else{$null}
  version=if($v.ProductVersion){[string]$v.ProductVersion}else{$null}
  productCode=$null
}
if([IO.Path]::GetExtension($p) -ieq '.msi'){
  try {
    $i=New-Object -ComObject WindowsInstaller.Installer
    $d=$i.OpenDatabase($p,0)
    foreach($key in @('ProductCode','ProductName','Manufacturer','ProductVersion')){
      $q=$d.OpenView("SELECT Value FROM Property WHERE Property = '$key'")
      $q.Execute()
      $row=$q.Fetch()
      if($row){
        $value=[string]$row.StringData(1)
        switch($key){'ProductCode'{$r.productCode=$value};'ProductName'{$r.name=$value};'Manufacturer'{$r.publisher=$value};'ProductVersion'{$r.version=$value}}
      }
      $q.Close()
    }
  } catch { $r.productCode=$null }
}
$r | ConvertTo-Json -Compress -Depth 3
"#;
const ELEVATE_MSI_SCRIPT: &str = r#"$ErrorActionPreference='Stop'; try { $p=Start-Process -FilePath $env:CASKLET_MSIEXEC_PATH -ArgumentList @('/i', ('"'+$env:CASKLET_INSTALLER_PATH+'"'), '/norestart') -WorkingDirectory $env:CASKLET_INSTALLER_DIR -Verb RunAs -PassThru -Wait; exit $p.ExitCode } catch { if ($_.Exception.NativeErrorCode -eq 1223) { exit 1223 }; exit 125 }"#;
const ELEVATE_SCRIPT: &str = r"$ErrorActionPreference='Stop'; try { $p=Start-Process -FilePath $env:CASKLET_INSTALLER_PATH -WorkingDirectory $env:CASKLET_INSTALLER_DIR -Verb RunAs -PassThru -Wait; exit $p.ExitCode } catch { if ($_.Exception.NativeErrorCode -eq 1223) { exit 1223 }; exit 125 }";
// Start-Process -Wait tracks descendants as well as the bootstrapper. Paths are
// environment values, never interpolated into executable PowerShell code.
const INSTALL_SCRIPT: &str = r"$ErrorActionPreference='Stop'; try { $p=Start-Process -FilePath $env:CASKLET_INSTALLER_PATH -WorkingDirectory $env:CASKLET_INSTALLER_DIR -PassThru -Wait; exit $p.ExitCode } catch { if($_.Exception.NativeErrorCode -eq 1223){exit 1223}; exit 125 }";

fn process_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else {
        PathBuf::from(path.strip_prefix(r"\\?\").unwrap_or(path))
    }
}
fn hold_verified(artifact: &InstallerInspection, path: &Path) -> Result<File, String> {
    #[cfg(windows)]
    let mut file = {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};
        // FILE_SHARE_READ permits the installer to read its bytes, but denies write and rename.
        OpenOptions::new()
            .read(true)
            .share_mode(0x0000_0001)
            .open(path)
            .map_err(|e| e.to_string())?
    };
    #[cfg(not(windows))]
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() != artifact.size_bytes {
        return Err("Installer identity changed before launch".into());
    }
    let mut hash = Sha256::new();
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total += u64::try_from(n).map_err(|e| e.to_string())?;
        if total > artifact.size_bytes {
            return Err("Installer grew before launch".into());
        }
        hash.update(&buf[..n]);
    }
    if total != artifact.size_bytes || format!("{:x}", hash.finalize()) != artifact.sha256 {
        return Err("Installer hash changed before launch".into());
    }
    Ok(file)
}
fn capture<R: Read>(mut reader: R) -> (Vec<u8>, bool) {
    let mut all = Vec::new();
    let mut buf = [0u8; 8192];
    let mut over = false;
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if all.len() + n > MAX_METADATA_BYTES {
                    over = true;
                } else {
                    all.extend_from_slice(&buf[..n]);
                }
            }
        }
    }
    (all, over)
}
fn metadata(path: &Path) -> Result<serde_json::Value, String> {
    let powershell = system_binary("WindowsPowerShell\\v1.0\\powershell.exe")?;
    let trusted_modules = powershell
        .parent()
        .ok_or("PowerShell directory missing")?
        .join("Modules");
    let fixed_script = format!(
        "Import-Module Microsoft.PowerShell.Security -ErrorAction Stop; Import-Module Microsoft.PowerShell.Utility -ErrorAction Stop; {INSPECT_SCRIPT}"
    );
    let mut child = super::windows_process::hide_console(&mut Command::new(powershell))
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "RemoteSigned",
            "-Command",
            &fixed_script,
        ])
        .env("CASKLET_MSIEXEC_PATH", system_binary("msiexec.exe")?)
        .env("CASKLET_INSTALLER_PATH", path)
        .env("PSModulePath", trusted_modules)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let out = child.stdout.take().ok_or("PowerShell stdout missing")?;
    let err = child.stderr.take().ok_or("PowerShell stderr missing")?;
    let stdout = thread::spawn(move || capture(out));
    let stderr = thread::spawn(move || capture(err));
    let start = Instant::now();
    let status = loop {
        if start.elapsed() > Duration::from_secs(30) {
            let _ = child.kill();
            break child.wait().map_err(|e| e.to_string())?;
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        thread::sleep(Duration::from_millis(100));
    };
    let (output, too_large) = stdout
        .join()
        .map_err(|_| "PowerShell stdout capture failed")?;
    let (_, err_large) = stderr
        .join()
        .map_err(|_| "PowerShell stderr capture failed")?;
    if !status.success() || too_large || err_large || start.elapsed() > Duration::from_secs(30) {
        return Err("Windows installer metadata inspection failed or exceeded limits".into());
    }
    serde_json::from_slice(&output).map_err(|_| "Invalid Windows installer metadata".into())
}
fn clean(value: Option<&str>, max: usize) -> Option<String> {
    value
        .map(str::trim)
        .filter(|v| {
            !v.is_empty()
                && v.chars().count() <= max
                && !v.chars().any(|c| {
                    c.is_control() || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
                })
        })
        .map(str::to_owned)
}
fn inspect(path: &str) -> Result<InstallerInspection, String> {
    let canonical = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
    let extension = canonical
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    let format = if extension.eq_ignore_ascii_case("msi") {
        "MSI"
    } else if extension.eq_ignore_ascii_case("exe") {
        "EXE"
    } else {
        return Err("Only .msi and .exe installers are supported".into());
    };
    let mut file = File::open(&canonical).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    if !file.metadata().map_err(|e| e.to_string())?.is_file()
        || length == 0
        || length > MAX_INSTALLER_BYTES
    {
        return Err("Installer must be a nonempty regular file within the size limit".into());
    }
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut count = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count += u64::try_from(n).map_err(|e| e.to_string())?;
        if count > MAX_INSTALLER_BYTES {
            return Err("Installer exceeded size limit".into());
        }
        hash.update(&buf[..n]);
    }
    if count != length {
        return Err("Installer size changed while hashing".into());
    }
    let data = metadata(&canonical)?;
    let raw_status = data
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("UnknownError");
    let signature_status = match raw_status {
        "Valid" => "VALID",
        "NotSigned" => "UNSIGNED",
        "HashMismatch" => "HASH_MISMATCH",
        "NotTrusted" => "UNTRUSTED",
        "NotSupportedFileFormat" | "Incompatible" => "UNSUPPORTED",
        _ => "UNKNOWN",
    };
    let signature_detail = clean(
        data.get("statusMessage")
            .and_then(serde_json::Value::as_str),
        512,
    );
    let signer = clean(data.get("signer").and_then(serde_json::Value::as_str), 512);
    let mut warnings=vec!["A signature identifies a signer and file integrity; it does not certify that an installer is safe. Casklet does not sandbox installers or guarantee cleanup.".into()];
    if signature_status != "VALID" {
        warnings.push(format!(
            "Signature status: {signature_status} ({raw_status})."
        ));
    }
    let product_code = clean(
        data.get("productCode").and_then(serde_json::Value::as_str),
        64,
    )
    .filter(|v| {
        v.len() == 38
            && v.chars().enumerate().all(|(i, c)| match i {
                0 => c == '{',
                37 => c == '}',
                9 | 14 | 19 | 24 => c == '-',
                _ => c.is_ascii_hexdigit(),
            })
    });
    if format == "MSI" && product_code.is_none() {
        warnings
            .push("MSI ProductCode could not be verified; candidates use weaker evidence.".into());
    }
    Ok(InstallerInspection {
        version: CONTRACT_VERSION,
        path: canonical.to_string_lossy().into_owned(),
        format: format.into(),
        sha256: format!("{:x}", hash.finalize()),
        size_bytes: count,
        signature_status: signature_status.into(),
        signature_detail,
        signer,
        product_name: clean(data.get("name").and_then(serde_json::Value::as_str), 240),
        publisher: clean(
            data.get("publisher").and_then(serde_json::Value::as_str),
            240,
        ),
        product_version: clean(data.get("version").and_then(serde_json::Value::as_str), 120),
        product_code,
        warnings,
    })
}
fn run(
    artifact: &InstallerInspection,
    allow_elevation: bool,
    cancel: &dyn Fn() -> bool,
    heartbeat: &dyn Fn(),
) -> Result<InstallerOutcome, String> {
    run_with_timeout(
        artifact,
        allow_elevation,
        cancel,
        heartbeat,
        Duration::from_hours(1),
    )
}
fn run_with_timeout(
    artifact: &InstallerInspection,
    allow_elevation: bool,
    cancel: &dyn Fn() -> bool,
    heartbeat: &dyn Fn(),
    timeout: Duration,
) -> Result<InstallerOutcome, String> {
    let launch_path = process_path(&artifact.path);
    let path = launch_path.as_path();
    let _held = hold_verified(artifact, path)?;
    let cwd = path.parent().ok_or("Installer directory missing")?;
    let mut command = if artifact.format == "MSI" && !allow_elevation {
        let mut c = Command::new(system_binary("msiexec.exe")?);
        c.arg("/i").arg(path).arg("/norestart");
        c
    } else {
        let powershell = system_binary("WindowsPowerShell\\v1.0\\powershell.exe")?;
        let trusted_modules = powershell
            .parent()
            .ok_or("PowerShell directory missing")?
            .join("Modules");
        let mut c = Command::new(powershell);
        c.args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "RemoteSigned",
            "-Command",
            if allow_elevation && artifact.format == "MSI" {
                ELEVATE_MSI_SCRIPT
            } else if allow_elevation {
                ELEVATE_SCRIPT
            } else {
                INSTALL_SCRIPT
            },
        ])
        .env("PSModulePath", trusted_modules)
        .env("CASKLET_MSIEXEC_PATH", system_binary("msiexec.exe")?)
        .env("CASKLET_INSTALLER_PATH", path)
        .env("CASKLET_INSTALLER_DIR", cwd);
        c
    };
    let mut child = super::windows_process::hide_console(&mut command)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Installer launch failed: {e}"))?;
    let start = Instant::now();
    let mut last = Instant::now()
        .checked_sub(Duration::from_secs(5))
        .ok_or("Invalid clock")?;
    loop {
        if cancel() {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(InstallerOutcome::Cancelled);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(InstallerOutcome::TimedOut);
        }
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let code = status.code().unwrap_or(-1);
            return Ok(
                if (allow_elevation || artifact.format == "EXE") && code == 1223 {
                    InstallerOutcome::ElevationDeclined
                } else if artifact.format == "EXE" {
                    InstallerOutcome::ProcessTreeExited(code)
                } else {
                    InstallerOutcome::Exited(code)
                },
            );
        }
        if last.elapsed() >= Duration::from_secs(5) {
            heartbeat();
            last = Instant::now();
        }
        thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn fake_executable_uses_exact_path_and_reports_timeout_and_cancellation() {
        let root =
            std::env::temp_dir().join(format!("casklet-installer & args-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let source = root.join("fake.rs");
        fs::write(
            &source,
            r#"
fn main() {
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().unwrap();
    if std::env::args().any(|arg| arg == "child") {
        std::thread::sleep(std::time::Duration::from_millis(800));
        std::fs::write(dir.join("child.marker"), b"complete").unwrap();
        return;
    }
    std::fs::write(dir.join("launched.marker"), b"yes").unwrap();
    let name = exe.file_name().unwrap().to_string_lossy();
    if name.contains("tree") {
        std::process::Command::new(&exe).arg("child").spawn().unwrap();
    }
    if name.contains("sleep") {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
"#,
        )
        .unwrap();
        let executable = root.join("installer & exact.exe");
        let status = Command::new("rustc")
            .args([source.to_str().unwrap(), "-o", executable.to_str().unwrap()])
            .status()
            .unwrap();
        assert!(status.success());
        let inspection = InstallerInspection {
            version: CONTRACT_VERSION,
            path: executable.to_string_lossy().into_owned(),
            format: "EXE".into(),
            sha256: format!("{:x}", Sha256::digest(fs::read(&executable).unwrap())),
            size_bytes: fs::metadata(&executable).unwrap().len(),
            signature_status: "UNSIGNED".into(),
            signature_detail: None,
            signer: None,
            product_name: None,
            publisher: None,
            product_version: None,
            product_code: None,
            warnings: vec![],
        };
        // Cold PowerShell/process startup on shared Windows runners can exceed
        // 15 seconds. This is a completion check, not a startup performance gate.
        // The explicit 250 ms timeout and immediate cancellation checks stay short.
        let completion_budget = Duration::from_secs(60);
        assert_eq!(
            run_with_timeout(&inspection, false, &|| false, &|| {}, completion_budget).unwrap(),
            InstallerOutcome::ProcessTreeExited(0)
        );
        assert!(root.join("launched.marker").exists());
        let tree = root.join("tree & exact.exe");
        fs::copy(&executable, &tree).unwrap();
        let mut bootstrapper = inspection.clone();
        bootstrapper.path = tree.to_string_lossy().into_owned();
        assert_eq!(
            run_with_timeout(&bootstrapper, false, &|| false, &|| {}, completion_budget).unwrap(),
            InstallerOutcome::ProcessTreeExited(0)
        );
        assert!(
            root.join("child.marker").exists(),
            "launch must wait for the bootstrapper's child"
        );
        let sleeper = root.join("sleep & exact.exe");
        fs::copy(&executable, &sleeper).unwrap();
        let mut sleeping = inspection.clone();
        sleeping.path = sleeper.to_string_lossy().into_owned();
        assert_eq!(
            run_with_timeout(
                &sleeping,
                false,
                &|| false,
                &|| {},
                Duration::from_millis(250)
            )
            .unwrap(),
            InstallerOutcome::TimedOut
        );
        assert_eq!(
            run_with_timeout(&sleeping, false, &|| true, &|| {}, Duration::from_secs(3)).unwrap(),
            InstallerOutcome::Cancelled
        );
    }
}
impl InstallerSource for LocalStorage {
    fn inspect_installer(&self, path: &str) -> Result<InstallerInspection, String> {
        #[cfg(windows)]
        {
            inspect(path)
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Err("Installer inspection is available on Windows only".into())
        }
    }
    fn run_installer(
        &self,
        artifact: &InstallerInspection,
        allow_elevation: bool,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<InstallerOutcome, String> {
        #[cfg(windows)]
        {
            run(artifact, allow_elevation, cancel, heartbeat)
        }
        #[cfg(not(windows))]
        {
            let _ = (artifact, allow_elevation, cancel, heartbeat);
            Err("Installer launch is available on Windows only".into())
        }
    }
}
