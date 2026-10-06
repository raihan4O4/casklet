//! Windows Installer process adapter: typed arguments, visible installer UI, no console.
use super::LocalStorage;
use casklet_application::managed_msi::{MsiUninstaller, product_code};

impl MsiUninstaller for LocalStorage {
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
