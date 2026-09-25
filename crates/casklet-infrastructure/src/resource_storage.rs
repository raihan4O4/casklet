//! Conservative Windows-first resource inspection and managed intake.
use super::LocalStorage;
use casklet_application::resource::{FileIdentity, IntakePaths, Observation, ResourceStorage};
use casklet_domain::resource::{IntakeMode, ResourceKind};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

#[cfg(windows)]
fn recordable_file_identity(path: &Path) -> Option<(String, same_file::Handle)> {
    let handle = winapi_util::Handle::from_path_any(path).ok()?;
    let info = winapi_util::file::information(&handle).ok()?;
    let file_identity = same_file::Handle::from_file(handle.as_file().try_clone().ok()?).ok()?;
    let mut hash = Sha256::new();
    let mut reader = handle.as_file();
    let mut buffer = vec![0_u8; 1024 * 1024].into_boxed_slice();
    loop {
        let n = reader.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Some((
        format!(
            "{:016x}:{:016x}:{:016x}:{:x}",
            info.volume_serial_number(),
            info.file_index(),
            info.creation_time()?,
            hash.finalize()
        ),
        file_identity,
    ))
}

#[cfg(not(windows))]
fn recordable_file_identity(_path: &Path) -> Option<(String, same_file::Handle)> {
    None
}

const MAX_ENTRIES: u64 = 100_000;
const MAX_DEPTH: usize = 64;

#[derive(Debug)]
struct LocalFileIdentity(same_file::Handle);
impl FileIdentity for LocalFileIdentity {
    fn matches_path(&self, path: &str) -> Result<bool, String> {
        let current = same_file::Handle::from_path(path).map_err(|e| e.to_string())?;
        Ok(self.0 == current)
    }
}

fn is_link(path: &Path) -> Result<bool, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        return Ok(true);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn check_components(path: &Path) -> Result<(), String> {
    for component in path.ancestors() {
        if component.as_os_str().is_empty() || !component.exists() {
            continue;
        }
        if is_link(component)? {
            return Err(format!(
                "link or reparse point is unsupported: {}",
                component.display()
            ));
        }
    }
    Ok(())
}

fn occupied(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.to_string()),
    }
}

fn availability_error(error: &std::io::Error) -> &'static str {
    match error.kind() {
        std::io::ErrorKind::NotFound => "MISSING",
        std::io::ErrorKind::PermissionDenied => "INACCESSIBLE",
        _ => "UNKNOWN",
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "bounded directory scan is one cohesive validation pass"
)]
fn inspect_path(source: &str, progress: &dyn Fn(u64, u64)) -> Result<Observation, String> {
    let input = Path::new(source);
    if source.is_empty() {
        return Err("Source path is empty".into());
    }
    #[cfg(windows)]
    if source.starts_with(r"\\.\")
        || (source.starts_with(r"\\?\")
            && !source
                .as_bytes()
                .get(4..6)
                .is_some_and(|drive| drive[0].is_ascii_alphabetic() && drive[1] == b':'))
    {
        return Err("Device namespace paths are unsupported".into());
    }
    let absolute = if input.is_absolute() {
        input.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(input)
    };
    check_components(&absolute)?;
    let path = fs::canonicalize(&absolute).map_err(|e| e.to_string())?;
    check_components(&path)?;
    let metadata = fs::metadata(&path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .ok_or("Filesystem root cannot be added")?
        .to_string_lossy()
        .into_owned();
    if name.is_empty() {
        return Err("Source has no file name".into());
    }
    let kind = if metadata.is_file() {
        ResourceKind::File
    } else if metadata.is_dir() {
        ResourceKind::Directory
    } else {
        return Err("Only files and directories are supported".into());
    };
    let mut size = 0_u64;
    let mut unknown = 0_u64;
    let mut entries = 0_u64;
    if kind == ResourceKind::File {
        size = metadata.len();
        progress(size, size);
    } else {
        let mut queue = VecDeque::from([(path.clone(), 0_usize)]);
        while let Some((directory, depth)) = queue.pop_front() {
            if depth > MAX_DEPTH {
                unknown += 1;
                continue;
            }
            let Ok(children) = fs::read_dir(&directory) else {
                unknown += 1;
                continue;
            };
            for child in children {
                if entries >= MAX_ENTRIES {
                    unknown += 1;
                    queue.clear();
                    break;
                }
                entries += 1;
                let Ok(child) = child else {
                    unknown += 1;
                    continue;
                };
                let child_path = child.path();
                let Ok(child_meta) = fs::symlink_metadata(&child_path) else {
                    unknown += 1;
                    continue;
                };
                if child_meta.file_type().is_symlink() || is_link(&child_path)? {
                    unknown += 1;
                    continue;
                }
                if child_meta.is_dir() {
                    queue.push_back((child_path, depth + 1));
                } else if child_meta.is_file() {
                    size = size.saturating_add(child_meta.len());
                } else {
                    unknown += 1;
                }
                if entries.is_multiple_of(128) {
                    progress(entries, 0);
                }
            }
        }
        progress(entries, 0);
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    let (recordable_identity, handle) = if kind == ResourceKind::File {
        match recordable_file_identity(&path) {
            Some((identity, handle)) => (Some(identity), handle),
            None => (
                None,
                same_file::Handle::from_path(&path).map_err(|e| e.to_string())?,
            ),
        }
    } else {
        (
            None,
            same_file::Handle::from_path(&path).map_err(|e| e.to_string())?,
        )
    };
    let file_identity = LocalFileIdentity(handle);
    if !file_identity.matches_path(&path.to_string_lossy())? {
        return Err("Path identity changed during inspection".into());
    }
    Ok(Observation {
        source_path: path.to_string_lossy().into_owned(),
        name,
        kind,
        size_bytes: Some(size),
        unknown_entries: unknown,
        identity: format!("{}:{size}:{entries}:{modified}", kind.code()),
        recordable_identity,
        file_identity: Arc::new(file_identity),
    })
}

fn copy_file(
    source: &Path,
    target: &Path,
    progress: &dyn Fn(u64, u64),
    completed: &mut u64,
    total: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    check_components(source)?;
    let mut reader = File::open(source).map_err(|e| e.to_string())?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|e| e.to_string())?;
    copy_stream(&mut reader, &mut writer, progress, completed, total, cancel)?;
    writer.sync_all().map_err(|e| e.to_string())
}

fn copy_stream(
    reader: &mut impl Read,
    writer: &mut impl Write,
    progress: &dyn Fn(u64, u64),
    completed: &mut u64,
    total: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let mut buffer = vec![0_u8; 1024 * 1024].into_boxed_slice();
    loop {
        if cancel() {
            return Err("Cancellation requested; staged data preserved".into());
        }
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        writer.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
        *completed = completed.saturating_add(u64::try_from(n).map_err(|e| e.to_string())?);
        progress(*completed, total);
        if cancel() {
            return Err("Cancellation requested; staged data preserved".into());
        }
    }
    Ok(())
}

fn copy_tree(
    source: &Path,
    target: &Path,
    progress: &dyn Fn(u64, u64),
    total: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    fs::create_dir(target).map_err(|e| e.to_string())?;
    let mut queue = VecDeque::from([(source.to_path_buf(), target.to_path_buf(), 0_usize)]);
    let mut completed = 0;
    let mut entries = 0_u64;
    while let Some((src_dir, dst_dir, depth)) = queue.pop_front() {
        if cancel() {
            return Err("Cancellation requested; staged data preserved".into());
        }
        check_components(&src_dir)?;
        for entry in fs::read_dir(&src_dir).map_err(|e| e.to_string())? {
            entries += 1;
            if entries > MAX_ENTRIES || depth >= MAX_DEPTH {
                return Err(
                    "Directory changed or exceeds the managed-copy scan limit; staging retained"
                        .into(),
                );
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let src = entry.path();
            let dst = dst_dir.join(entry.file_name());
            if is_link(&src)? {
                return Err(
                    "Directory contains a link or reparse point; managed copy stopped".into(),
                );
            }
            let metadata = fs::symlink_metadata(&src).map_err(|e| e.to_string())?;
            if metadata.is_dir() {
                fs::create_dir(&dst).map_err(|e| e.to_string())?;
                queue.push_back((src, dst, depth + 1));
            } else if metadata.is_file() {
                copy_file(&src, &dst, progress, &mut completed, total, cancel)?;
            } else {
                return Err("Directory contains an unsupported entry".into());
            }
        }
    }
    Ok(())
}

impl ResourceStorage for LocalStorage {
    fn inspect(&self, source: &str, progress: &dyn Fn(u64, u64)) -> Result<Observation, String> {
        inspect_path(source, progress)
    }
    fn availability(&self, path: &str, kind: ResourceKind) -> &'static str {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if is_link(Path::new(path)).unwrap_or(true)
                    || (kind == ResourceKind::File && !metadata.is_file())
                    || (kind == ResourceKind::Directory && !metadata.is_dir())
                {
                    "CHANGED"
                } else {
                    "PRESENT"
                }
            }
            Err(error) => availability_error(&error),
        }
    }
    fn allocate(&self, cask: &casklet_domain::Cask, resource_id: &str, name: &str) -> IntakePaths {
        let extension = Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| e.len() <= 10 && e.chars().all(|c| c.is_ascii_alphanumeric()))
            .map_or_else(String::new, |e| format!(".{e}"));
        let root = Path::new(&cask.managed_storage_root);
        IntakePaths {
            staging: root
                .join(".casklet")
                .join("staging")
                .join(resource_id)
                .to_string_lossy()
                .into_owned(),
            final_path: root
                .join("content")
                .join("other")
                .join(format!("{resource_id}{extension}"))
                .to_string_lossy()
                .into_owned(),
        }
    }
    fn materialize(
        &self,
        observed: &Observation,
        paths: &IntakePaths,
        mode: IntakeMode,
        progress: &dyn Fn(u64, u64),
        cancel: &dyn Fn() -> bool,
    ) -> Result<Option<String>, String> {
        let source = Path::new(&observed.source_path);
        let staging = Path::new(&paths.staging);
        let final_path = Path::new(&paths.final_path);
        let root = &self.root;
        check_components(source)?;
        check_components(root)?;
        if occupied(final_path)? || occupied(staging)? {
            return Err("Managed destination already exists".into());
        }
        let stage_parent = staging.parent().ok_or("Invalid staging path")?;
        let final_parent = final_path.parent().ok_or("Invalid managed path")?;
        fs::create_dir_all(stage_parent).map_err(|e| e.to_string())?;
        fs::create_dir_all(final_parent).map_err(|e| e.to_string())?;
        check_components(stage_parent)?;
        check_components(final_parent)?;
        if fs::canonicalize(stage_parent)
            .map_err(|e| e.to_string())?
            .starts_with(source)
            || fs::canonicalize(final_parent)
                .map_err(|e| e.to_string())?
                .starts_with(source)
        {
            return Err("Source contains the managed destination; intake stopped".into());
        }
        let rechecked = inspect_path(&observed.source_path, &|_, _| {})?;
        if rechecked.identity != observed.identity
            || !observed.file_identity.matches_path(&observed.source_path)?
            || rechecked.unknown_entries > 0
        {
            return Err("Source changed or contains unverified entries; source preserved".into());
        }
        match mode {
            IntakeMode::Reference => return Err("Reference has no managed materialization".into()),
            IntakeMode::Move => {
                if cancel() {
                    return Err("Cancellation requested before move".into());
                }
                check_components(source)?;
                if !observed.file_identity.matches_path(&observed.source_path)? {
                    return Err("Source identity changed before move; source preserved".into());
                }
                fs::rename(source, staging).map_err(|e| {
                    format!("Atomic move failed; source remains at its original path: {e}")
                })?;
                if !observed.file_identity.matches_path(&paths.staging)? {
                    return Err("Moved item identity changed; staging retained for recovery".into());
                }
                if cancel() {
                    return Err("Cancellation requested; staged data preserved".into());
                }
            }
            IntakeMode::Copy => {
                if observed.kind == ResourceKind::File {
                    let mut completed = 0;
                    copy_file(
                        source,
                        staging,
                        progress,
                        &mut completed,
                        observed.size_bytes.unwrap_or(0),
                        cancel,
                    )?;
                } else {
                    copy_tree(
                        source,
                        staging,
                        progress,
                        observed.size_bytes.unwrap_or(0),
                        cancel,
                    )?;
                }
            }
        }
        if cancel() {
            return Err("Cancellation requested; staged data preserved".into());
        }
        check_components(staging)?;
        let verified = inspect_path(&staging.to_string_lossy(), &|_, _| {})?;
        if verified.size_bytes != observed.size_bytes || verified.unknown_entries > 0 {
            return Err(
                "Managed staging verification failed; staging retained for recovery".into(),
            );
        }
        if occupied(final_path)? {
            return Err("Managed destination appeared during intake".into());
        }
        if cancel() {
            return Err("Cancellation requested; staged data preserved".into());
        }
        fs::rename(staging, final_path).map_err(|e| e.to_string())?;
        if !verified.file_identity.matches_path(&paths.final_path)? {
            return Err("Managed item identity changed after rename".into());
        }
        progress(
            observed.size_bytes.unwrap_or(0),
            observed.size_bytes.unwrap_or(0),
        );
        Ok(verified.recordable_identity)
    }
    fn finish_staged(
        &self,
        cask: &casklet_domain::Cask,
        resource: &casklet_domain::resource::Resource,
        paths: &IntakePaths,
        observed: &Observation,
    ) -> Result<Option<String>, String> {
        let expected = self.allocate(cask, &resource.id, &resource.name);
        if expected.staging != paths.staging || expected.final_path != paths.final_path {
            return Err("Recovery paths no longer match managed allocation".into());
        }
        let staging = Path::new(&paths.staging);
        let final_path = Path::new(&paths.final_path);
        check_components(staging)?;
        check_components(final_path)?;
        let final_parent = final_path.parent().ok_or("Invalid managed destination")?;
        fs::create_dir_all(final_parent).map_err(|e| e.to_string())?;
        check_components(final_parent)?;
        if occupied(final_path)? {
            return Err("Managed destination is occupied; staged item preserved".into());
        }
        if !observed.file_identity.matches_path(&paths.staging)? {
            return Err("Staged item identity changed; files preserved".into());
        }
        fs::rename(staging, final_path).map_err(|e| e.to_string())?;
        if !observed.file_identity.matches_path(&paths.final_path)? {
            return Err("Managed item identity changed after recovery rename".into());
        }
        Ok(observed.recordable_identity.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FullAfterOneByte(usize);
    impl Write for FullAfterOneByte {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0 == 0 {
                return Err(std::io::Error::from_raw_os_error(112));
            }
            let count = bytes.len().min(self.0);
            self.0 -= count;
            Ok(count)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn full_destination_stops_copy_without_claiming_completion() {
        let mut source = std::io::Cursor::new(b"source data");
        let mut destination = FullAfterOneByte(1);
        let mut completed = 0;
        let error = copy_stream(
            &mut source,
            &mut destination,
            &|_, _| {},
            &mut completed,
            11,
            &|| false,
        )
        .unwrap_err();
        assert!(!error.is_empty());
        assert_eq!(completed, 0);
        assert_eq!(source.into_inner(), b"source data");
    }

    #[test]
    fn inaccessible_path_is_distinct_from_missing_path() {
        assert_eq!(
            availability_error(&std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            "INACCESSIBLE"
        );
        assert_eq!(
            availability_error(&std::io::Error::from(std::io::ErrorKind::NotFound)),
            "MISSING"
        );
    }
}
