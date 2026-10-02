//! System executable resolution avoids current-directory and PATH lookup.
use std::path::PathBuf;

pub(super) fn system_binary(name: &str) -> Result<PathBuf, String> {
    let root = std::env::var_os("SystemRoot").ok_or("Windows system directory unavailable")?;
    let root = PathBuf::from(root)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let path = root
        .join("System32")
        .join(name)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !path.starts_with(root.join("System32")) {
        return Err("Windows system binary escaped System32".into());
    }
    let text = path.to_string_lossy();
    Ok(PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)))
}
