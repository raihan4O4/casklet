//! Bounded ZIP inspection and extraction. Archive entry names are never trusted as paths.
use super::{LocalStorage, resource_storage::check_components};
use casklet_application::portable::{PortableInspection, PortablePaths, PortableProvider};
use casklet_domain::Cask;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};
use zip::{CompressionMethod, ZipArchive};

const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 10_000;
const MAX_DEPTH: usize = 16;
const MAX_RATIO: u64 = 1_000;

#[derive(Debug, Clone)]
pub struct ZipSummary {
    pub sha256: String,
    pub file_count: u32,
    pub expanded_bytes: u64,
    pub executables: Vec<String>,
}

fn clean_entry(name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || name.len() > 240
        || name.starts_with('/')
        || name.contains(['\\', ':', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err("Archive contains an unsafe path".into());
    }
    let trimmed = name.trim_end_matches('/');
    let mut output = PathBuf::new();
    let mut depth = 0;
    for part in trimmed.split('/') {
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.ends_with([' ', '.'])
            || part.contains(['<', '>', '"', '|', '?', '*'])
        {
            return Err("Archive contains an unsafe path component".into());
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            return Err("Archive uses a reserved device name".into());
        }
        output.push(part);
        depth += 1;
        if depth > MAX_DEPTH {
            return Err("Archive path is too deep".into());
        }
    }
    if output
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Archive path is unsafe".into());
    }
    Ok(output)
}

fn sha256_file(file: &mut File) -> Result<String, String> {
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hash.finalize()))
}

fn open_archive(path: &Path) -> Result<(ZipArchive<File>, String), String> {
    check_components(path)?;
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size == 0 || size > MAX_ARCHIVE_BYTES {
        return Err("ZIP archive exceeds the supported size".into());
    }
    let hash = sha256_file(&mut file)?;
    let archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    Ok((archive, hash))
}

fn inspect_archive(archive: &mut ZipArchive<File>, sha256: String) -> Result<ZipSummary, String> {
    if archive.is_empty() || archive.len() > MAX_FILES {
        return Err("ZIP entry count is unsupported".into());
    }
    let mut seen = HashSet::new();
    let mut files = 0_u32;
    let mut total = 0_u64;
    let mut executables = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let relative = clean_entry(entry.name())?;
        let key = relative.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            return Err("ZIP contains duplicate or case-colliding paths".into());
        }
        if entry.is_symlink()
            || (!entry.is_dir() && !entry.is_file())
            || entry.unix_mode().is_some_and(|mode| {
                let kind = mode & 0o170_000;
                kind != 0 && kind != 0o100_000 && kind != 0o040_000
            })
        {
            return Err("ZIP contains a link or special entry".into());
        }
        if !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        ) {
            return Err("Unsupported ZIP compression method".into());
        }
        if entry.is_dir() {
            continue;
        }
        files = files.saturating_add(1);
        let size = entry.size();
        if size > MAX_FILE_BYTES
            || (size > 0
                && (entry.compressed_size() == 0
                    || size / entry.compressed_size().max(1) > MAX_RATIO))
        {
            return Err("ZIP file exceeds size or expansion limit".into());
        }
        total = total
            .checked_add(size)
            .ok_or("ZIP expanded size overflow")?;
        if total > MAX_EXPANDED_BYTES {
            return Err("ZIP expanded size exceeds limit".into());
        }
        if relative
            .extension()
            .and_then(|v| v.to_str())
            .is_some_and(|v| v.eq_ignore_ascii_case("exe"))
        {
            executables.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    if files == 0 || executables.is_empty() {
        return Err("ZIP contains no portable executable".into());
    }
    executables.sort();
    Ok(ZipSummary {
        sha256,
        file_count: files,
        expanded_bytes: total,
        executables,
    })
}

/// # Errors
/// Rejects unsafe, unsupported, oversized, or unreadable ZIP archives.
pub fn inspect_zip(path: &Path) -> Result<ZipSummary, String> {
    let (mut archive, hash) = open_archive(path)?;
    inspect_archive(&mut archive, hash)
}

/// Extracts only an archive matching the inspected hash into a new operation-owned staging directory.
/// # Errors
/// Rejects changed archives, unsafe entries, exhausted limits, or interrupted extraction.
pub fn extract_zip(
    path: &Path,
    expected_sha256: &str,
    selected_exe: &str,
    staging: &Path,
    progress: &dyn Fn(u64, u64),
    cancel: &dyn Fn() -> bool,
) -> Result<(u64, String), String> {
    let (mut archive, hash) = open_archive(path)?;
    if hash != expected_sha256 {
        return Err("ZIP changed since preview".into());
    }
    let summary = inspect_archive(&mut archive, hash)?;
    if !summary.executables.iter().any(|e| e == selected_exe) {
        return Err("Selected executable is not in the inspected ZIP".into());
    }
    let parent = staging.parent().ok_or("Invalid staging path")?;
    check_components(parent)?;
    if fs::symlink_metadata(staging).is_ok() {
        return Err("Staging path is occupied".into());
    }
    fs::create_dir(staging).map_err(|e| e.to_string())?;
    let mut completed = 0_u64;
    for index in 0..archive.len() {
        if cancel() {
            return Err("Portable installation cancelled; staging preserved".into());
        }
        let mut entry = archive.by_index(index).map_err(|e| e.to_string())?;
        let relative = clean_entry(entry.name())?;
        let target = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        let target_parent = target.parent().ok_or("Invalid ZIP destination")?;
        fs::create_dir_all(target_parent).map_err(|e| e.to_string())?;
        check_components(target_parent)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| e.to_string())?;
        let expected_size = entry.size();
        let copied = std::io::copy(&mut entry.by_ref().take(expected_size + 1), &mut output)
            .map_err(|e| e.to_string())?;
        if copied != entry.size() {
            return Err("ZIP content size disagrees with directory".into());
        }
        output.flush().map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        completed = completed
            .checked_add(copied)
            .ok_or("Expanded size overflow")?;
        if completed > MAX_EXPANDED_BYTES {
            return Err("ZIP expanded size exceeds limit".into());
        }
        progress(completed, summary.expanded_bytes);
    }
    let exe = staging.join(selected_exe.replace('/', std::path::MAIN_SEPARATOR_STR));
    check_components(&exe)?;
    let mut executable = File::open(exe).map_err(|e| e.to_string())?;
    let exe_hash = sha256_file(&mut executable)?;
    Ok((completed, exe_hash))
}

/// # Errors
/// Refuses to launch a changed or missing installed executable.
pub fn launch_verified(root: &Path, relative: &str, expected_hash: &str) -> Result<(), String> {
    let relative = clean_entry(relative)?;
    let executable = root.join(relative);
    check_components(&executable)?;
    let mut file = File::open(&executable).map_err(|e| e.to_string())?;
    if sha256_file(&mut file)? != expected_hash {
        return Err("Installed executable changed".into());
    }
    Command::new(&executable)
        .current_dir(root)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl PortableProvider for LocalStorage {
    fn inspect_zip(&self, path: &str) -> Result<PortableInspection, String> {
        let item = inspect_zip(Path::new(path))?;
        Ok(PortableInspection {
            sha256: item.sha256,
            file_count: item.file_count,
            expanded_bytes: item.expanded_bytes,
            executable_paths: item.executables,
        })
    }
    fn allocate_portable(
        &self,
        cask: &Cask,
        operation_id: &str,
        resource_id: &str,
    ) -> PortablePaths {
        let root = Path::new(&cask.managed_storage_root);
        PortablePaths {
            staging: root
                .join(".casklet")
                .join("staging")
                .join(operation_id)
                .to_string_lossy()
                .into_owned(),
            final_path: root
                .join("content")
                .join("other")
                .join(resource_id)
                .to_string_lossy()
                .into_owned(),
        }
    }
    fn extract_zip(
        &self,
        path: &str,
        expected_hash: &str,
        selected_exe: &str,
        staging: &str,
        progress: &dyn Fn(u64, u64),
        cancel: &dyn Fn() -> bool,
    ) -> Result<(u64, String), String> {
        let parent = Path::new(staging).parent().ok_or("Invalid staging path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        extract_zip(
            Path::new(path),
            expected_hash,
            selected_exe,
            Path::new(staging),
            progress,
            cancel,
        )
    }
    fn finalize_portable(&self, staging: &str, final_path: &str) -> Result<(), String> {
        let staging = Path::new(staging);
        let final_path = Path::new(final_path);
        let parent = final_path.parent().ok_or("Invalid installation path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        check_components(staging)?;
        check_components(parent)?;
        if fs::symlink_metadata(final_path).is_ok() {
            return Err("Installation destination is occupied".into());
        }
        fs::rename(staging, final_path).map_err(|e| e.to_string())
    }
    fn launch_portable(
        &self,
        root: &str,
        relative_exe: &str,
        expected_hash: &str,
    ) -> Result<(), String> {
        launch_verified(Path::new(root), relative_exe, expected_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn fixture(name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("casklet-zip-{name}-{}.zip", uuid::Uuid::new_v4()));
        let mut writer = ZipWriter::new(File::create(&path).unwrap());
        for (entry, bytes) in entries {
            writer
                .start_file(*entry, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn valid_zip_inspects_and_extracts_selected_executable() {
        let zip = fixture(
            "good",
            &[("bin/app.exe", b"MZ-demo"), ("readme.txt", b"hello")],
        );
        let preview = inspect_zip(&zip).unwrap();
        assert_eq!(preview.executables, vec!["bin/app.exe"]);
        let stage =
            std::env::temp_dir().join(format!("casklet-zip-stage-{}", uuid::Uuid::new_v4()));
        let (size, hash) = extract_zip(
            &zip,
            &preview.sha256,
            "bin/app.exe",
            &stage,
            &|_, _| {},
            &|| false,
        )
        .unwrap();
        assert_eq!(size, 12);
        assert_eq!(hash, format!("{:x}", Sha256::digest(b"MZ-demo")));
        fs::remove_dir_all(stage).unwrap();
        fs::remove_file(zip).unwrap();
    }

    #[test]
    fn rejects_traversal_links_case_collisions_and_bombs() {
        for (name, entries) in [
            ("traversal", vec![("../escape.exe", b"x".as_slice())]),
            ("backslash", vec![("..\\escape.exe", b"x".as_slice())]),
            (
                "collision",
                vec![("App.exe", b"x".as_slice()), ("app.exe", b"y".as_slice())],
            ),
            ("reserved", vec![("CON.exe", b"x".as_slice())]),
        ] {
            let zip = fixture(name, &entries);
            assert!(inspect_zip(&zip).is_err(), "{name}");
            fs::remove_file(zip).unwrap();
        }
        let bomb = fixture("bomb", &[("app.exe", &vec![0_u8; 4 * 1024 * 1024])]);
        assert!(inspect_zip(&bomb).is_err());
        fs::remove_file(bomb).unwrap();
        let link =
            std::env::temp_dir().join(format!("casklet-zip-link-{}.zip", uuid::Uuid::new_v4()));
        let mut writer = ZipWriter::new(File::create(&link).unwrap());
        writer
            .add_symlink("app.exe", "../outside", SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
        assert!(inspect_zip(&link).is_err());
        fs::remove_file(link).unwrap();
    }
}
