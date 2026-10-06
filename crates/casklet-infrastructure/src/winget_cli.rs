//! Typed `WinGet` process adapter with bounded capture, timeouts and cooperative cancellation.
use super::LocalStorage;
use casklet_application::winget::{InstalledPackage, WingetProvider};
use casklet_contracts::{CONTRACT_VERSION, WingetCapabilities};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const OUTPUT_LIMIT: usize = 256 * 1024;
const EXPORT_LIMIT: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
struct WingetCli {
    executable: PathBuf,
}

#[cfg(windows)]
fn resolve_winget() -> Result<WingetCli, String> {
    let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA unavailable")?;
    let programs = std::env::var_os("ProgramFiles").ok_or("ProgramFiles unavailable")?;
    let alias = PathBuf::from(local)
        .join("Microsoft")
        .join("WindowsApps")
        .join("winget.exe");
    let actual =
        fs::canonicalize(alias).map_err(|_| "WinGet app execution alias is unavailable")?;
    let trusted = PathBuf::from(programs).join("WindowsApps");
    let actual_text = actual.to_string_lossy().to_lowercase();
    let trusted_text = trusted.to_string_lossy().to_lowercase();
    let actual_text = actual_text.trim_start_matches(r"\\?\");
    if !actual_text.starts_with(&(trusted_text + "\\microsoft.desktopappinstaller_"))
        || !actual.is_file()
    {
        return Err("WinGet did not resolve into the Desktop App Installer package".into());
    }
    Ok(WingetCli { executable: actual })
}

#[cfg(not(windows))]
fn resolve_winget() -> Result<WingetCli, String> {
    Err("WinGet is available on Windows only".into())
}

fn sanitize(raw: &[u8]) -> String {
    let mut result = String::new();
    let decoded = String::from_utf8_lossy(raw);
    let mut chars = decoded.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        if ch == '\n'
            || ch == '\t'
            || (!ch.is_control() && !matches!(ch,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}'))
        {
            result.push(ch);
        }
        if result.len() >= OUTPUT_LIMIT {
            break;
        }
    }
    result
}

fn capture(mut reader: impl Read) -> (Vec<u8>, bool) {
    let mut out = Vec::new();
    let mut buf = [0u8; 8192];
    let mut over = false;
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if out.len() + n > OUTPUT_LIMIT {
                    over = true;
                }
                let keep = OUTPUT_LIMIT.saturating_sub(out.len()).min(n);
                out.extend_from_slice(&buf[..keep]);
            }
        }
    }
    (out, over)
}

impl WingetCli {
    fn hash(&self) -> Result<String, String> {
        let mut file = File::open(&self.executable).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = file.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        Ok(format!("{:x}", hash.finalize()))
    }
    fn run(
        &self,
        args: &[&str],
        timeout: Duration,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<String, String> {
        let initial = self.hash()?;
        let cwd = self.executable.parent().ok_or("Invalid WinGet path")?;
        let mut child = super::windows_process::hide_console(&mut Command::new(&self.executable))
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().ok_or("Missing WinGet stdout")?;
        let stderr = child.stderr.take().ok_or("Missing WinGet stderr")?;
        let output = thread::spawn(move || capture(stdout));
        let errors = thread::spawn(move || capture(stderr));
        let start = Instant::now();
        let mut last_heartbeat = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .ok_or("Invalid clock")?;
        let mut reason = None;
        let status = loop {
            if cancel() {
                reason = Some("CANCELLED");
                let _ = child.kill();
                break child.wait().map_err(|e| e.to_string())?;
            }
            if start.elapsed() > timeout {
                reason = Some("TIMED_OUT");
                let _ = child.kill();
                break child.wait().map_err(|e| e.to_string())?;
            }
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                break status;
            }
            if last_heartbeat.elapsed() >= Duration::from_secs(5) {
                heartbeat();
                last_heartbeat = Instant::now();
            }
            thread::sleep(Duration::from_millis(250));
        };
        let (out, out_over) = output.join().map_err(|_| "WinGet capture failed")?;
        let (err, err_over) = errors.join().map_err(|_| "WinGet capture failed")?;
        if let Some(reason) = reason {
            return Err(reason.into());
        }
        if out_over || err_over {
            return Err("WinGet output exceeded the capture limit".into());
        }
        if self.hash()? != initial {
            return Err("WinGet executable changed during invocation".into());
        }
        if !status.success() {
            return Err(format!(
                "WinGet exited with status {:?}: {}",
                status.code(),
                sanitize(&err).chars().take(2048).collect::<String>()
            ));
        }
        Ok(sanitize(&out))
    }
    fn export(&self) -> Result<Vec<InstalledPackage>, String> {
        let target = std::env::temp_dir().join(format!("casklet-winget-{}.json", Uuid::new_v4()));
        let target_text = target.to_string_lossy().into_owned();
        let result = self.run(
            &[
                "export",
                "--output",
                &target_text,
                "--source",
                "winget",
                "--include-versions",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
            Duration::from_secs(120),
            &|| false,
            &|| {},
        );
        if let Err(error) = result {
            let _ = fs::remove_file(&target);
            return Err(error);
        }
        let mut bytes = Vec::new();
        File::open(&target)
            .map_err(|e| e.to_string())?
            .take(EXPORT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        let _ = fs::remove_file(&target);
        if bytes.len() as u64 > EXPORT_LIMIT {
            return Err("WinGet export exceeded size limit".into());
        }
        parse_export(&bytes)
    }
}

fn parse_export(bytes: &[u8]) -> Result<Vec<InstalledPackage>, String> {
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|_| "Invalid WinGet export JSON")?;
    let sources = value
        .get("Sources")
        .and_then(serde_json::Value::as_array)
        .ok_or("WinGet export omitted sources")?;
    let mut packages = Vec::new();
    for source in sources {
        let name = source
            .get("SourceDetails")
            .and_then(|v| v.get("Name"))
            .and_then(serde_json::Value::as_str)
            .ok_or("WinGet export omitted source name")?;
        if name != "winget" {
            continue;
        }
        let list = source
            .get("Packages")
            .and_then(serde_json::Value::as_array)
            .ok_or("WinGet export omitted package list")?;
        for package in list {
            let id = package
                .get("PackageIdentifier")
                .and_then(serde_json::Value::as_str)
                .ok_or("Invalid WinGet package ID")?;
            let version = package
                .get("Version")
                .and_then(serde_json::Value::as_str)
                .ok_or("WinGet export omitted installed version")?;
            if id.len() > 120 || version.len() > 80 {
                return Err("WinGet export contains oversized identity".into());
            }
            packages.push(InstalledPackage {
                id: id.into(),
                version: version.into(),
            });
            if packages.len() > 20_000 {
                return Err("WinGet export contains too many packages".into());
            }
        }
    }
    Ok(packages)
}

impl WingetProvider for LocalStorage {
    fn capabilities(&self) -> WingetCapabilities {
        match resolve_winget() {
            Ok(_) => WingetCapabilities {
                version: CONTRACT_VERSION,
                search: true,
                install: true,
                uninstall: cfg!(feature = "reviewed-removal"),
                reason: None,
            },
            Err(error) => WingetCapabilities {
                version: CONTRACT_VERSION,
                search: false,
                install: false,
                uninstall: false,
                reason: Some(error),
            },
        }
    }
    fn search(&self, query: &str) -> Result<String, String> {
        resolve_winget()?.run(
            &[
                "search",
                query,
                "--source",
                "winget",
                "--count",
                "50",
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
            Duration::from_secs(120),
            &|| false,
            &|| {},
        )
    }
    fn show(&self, id: &str, version: &str) -> Result<(String, String), String> {
        let cli = resolve_winget()?;
        let hash = cli.hash()?;
        let text = cli.run(
            &[
                "show",
                "--id",
                id,
                "--exact",
                "--source",
                "winget",
                "--version",
                version,
                "--accept-source-agreements",
                "--disable-interactivity",
            ],
            Duration::from_secs(120),
            &|| false,
            &|| {},
        )?;
        Ok((text, hash))
    }
    fn installed_packages(&self) -> Result<Vec<InstalledPackage>, String> {
        resolve_winget()?.export()
    }
    fn install(
        &self,
        id: &str,
        version: &str,
        accept_agreements: bool,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<(), String> {
        if !accept_agreements {
            return Err("Package agreements not accepted".into());
        }
        resolve_winget()?
            .run(
                &[
                    "install",
                    "--id",
                    id,
                    "--exact",
                    "--source",
                    "winget",
                    "--version",
                    version,
                    "--no-upgrade",
                    "--silent",
                    "--accept-package-agreements",
                    "--accept-source-agreements",
                    "--disable-interactivity",
                ],
                Duration::from_mins(30),
                cancel,
                heartbeat,
            )
            .map(|_| ())
    }
    fn uninstall(
        &self,
        id: &str,
        version: &str,
        cancel: &dyn Fn() -> bool,
        heartbeat: &dyn Fn(),
    ) -> Result<(), String> {
        resolve_winget()?
            .run(
                &[
                    "uninstall",
                    "--id",
                    id,
                    "--exact",
                    "--source",
                    "winget",
                    "--version",
                    version,
                    "--silent",
                    "--disable-interactivity",
                ],
                Duration::from_mins(30),
                cancel,
                heartbeat,
            )
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fake_cli() -> WingetCli {
        let dir = std::env::temp_dir().join(format!("casklet-fake-winget-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let source = dir.join("fake.rs");
        fs::write(&source,r##"use std::{env,fs,thread,time::Duration};
fn main(){let args:Vec<String>=env::args().collect();let marker=env::current_exe().unwrap().parent().unwrap().join("installed.marker");
match args.get(1).map(String::as_str){
Some("search")=>print!("Found Fake [Vendor.Fake]\x1b[31m"),
Some("show")=>print!("Found Fake [Vendor.Fake]\nVersion: 1.0\n"),
Some("export")=>{let position=args.iter().position(|a|a=="--output").unwrap();let packages=if marker.exists(){r#"[{"PackageIdentifier":"Vendor.Fake","Version":"1.0"}]"#}else{"[]"};fs::write(&args[position+1],format!(r#"{{"Sources":[{{"SourceDetails":{{"Name":"winget"}},"Packages":{packages}}}]}}"#)).unwrap();},
Some("install")=>{assert!(args.windows(2).any(|a|a[0]=="--id"&&a[1]=="Vendor.Fake"));fs::write(marker,"yes").unwrap();},
Some("uninstall")=>{let _=fs::remove_file(marker);},
Some("sleep")=>thread::sleep(Duration::from_secs(5)),
_=>std::process::exit(2)} }"##).unwrap();
        let executable = dir.join(if cfg!(windows) { "fake.exe" } else { "fake" });
        let status = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .status()
            .unwrap();
        assert!(status.success());
        WingetCli { executable }
    }
    #[test]
    fn export_requires_structured_source_and_version() {
        let packages=parse_export(br#"{"Sources":[{"SourceDetails":{"Name":"winget"},"Packages":[{"PackageIdentifier":"Vendor.App","Version":"1.0"}]}]}"#).unwrap();
        assert_eq!(
            packages,
            vec![InstalledPackage {
                id: "Vendor.App".into(),
                version: "1.0".into()
            }]
        );
        assert!(parse_export(br#"{"Sources":[{"SourceDetails":{"Name":"winget"},"Packages":[{"PackageIdentifier":"Vendor.App"}]}]}"#).is_err());
    }
    #[test]
    fn output_sanitizes_control_sequences() {
        assert_eq!(
            sanitize(b"Found App [Vendor.App]\x1b[31m\r\n"),
            "Found App [Vendor.App]\n"
        );
    }
    #[test]
    fn fake_executable_enforces_typed_arguments_and_bounded_execution() {
        let cli = fake_cli();
        let text = cli
            .run(
                &["search", "Fake"],
                Duration::from_secs(5),
                &|| false,
                &|| {},
            )
            .unwrap();
        assert!(!text.contains('\u{1b}'));
        assert!(cli.export().unwrap().is_empty());
        cli.run(
            &["install", "--id", "Vendor.Fake"],
            Duration::from_secs(5),
            &|| false,
            &|| {},
        )
        .unwrap();
        assert_eq!(
            cli.export().unwrap(),
            vec![InstalledPackage {
                id: "Vendor.Fake".into(),
                version: "1.0".into()
            }]
        );
        assert_eq!(
            cli.run(&["sleep"], Duration::from_millis(100), &|| false, &|| {})
                .unwrap_err(),
            "TIMED_OUT"
        );
        assert_eq!(
            cli.run(&["sleep"], Duration::from_secs(5), &|| true, &|| {})
                .unwrap_err(),
            "CANCELLED"
        );
        cli.run(&["uninstall"], Duration::from_secs(5), &|| false, &|| {})
            .unwrap();
        assert!(cli.export().unwrap().is_empty());
    }
}
