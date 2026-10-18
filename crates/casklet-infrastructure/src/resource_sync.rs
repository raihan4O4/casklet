//! Bounded metadata reconciliation. No content hashes or ownership changes.
use super::resource_storage::check_components;
use casklet_application::resource::ResourceMeasurement;
use casklet_contracts::ResourceContentEntry;
use casklet_domain::resource::ResourceKind;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

#[derive(Debug)]
struct DirectoryListing {
    modified: Option<SystemTime>,
    children: Vec<PathBuf>,
}

#[derive(Debug)]
struct CachedTree {
    directories: HashMap<PathBuf, DirectoryListing>,
    observed: ResourceMeasurement,
    checked: Instant,
    reconciled: Instant,
    interval: Duration,
    root_identity: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct MeasurementCache {
    trees: HashMap<String, CachedTree>,
}

fn unavailable(state: &str) -> ResourceMeasurement {
    ResourceMeasurement {
        availability: state.into(),
        size_bytes: None,
        unknown_entries: 0,
        total_entries: 0,
        entries: Vec::new(),
    }
}

fn root_identity(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        super::resource_storage::directory_object_identity(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

impl MeasurementCache {
    pub(super) fn measure(
        &mut self,
        path: &str,
        kind: ResourceKind,
        force: bool,
    ) -> ResourceMeasurement {
        let root = Path::new(path);
        if check_components(root).is_err() {
            self.trees.remove(path);
            return unavailable("CHANGED");
        }
        let metadata = match fs::symlink_metadata(root) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.trees.remove(path);
                return unavailable(match error.kind() {
                    std::io::ErrorKind::NotFound => "MISSING",
                    std::io::ErrorKind::PermissionDenied => "INACCESSIBLE",
                    _ => "UNKNOWN",
                });
            }
        };
        if metadata.is_file() && kind == ResourceKind::File {
            return ResourceMeasurement {
                availability: "PRESENT".into(),
                size_bytes: Some(metadata.len()),
                unknown_entries: 0,
                total_entries: 0,
                entries: Vec::new(),
            };
        }
        if !metadata.is_dir() || kind != ResourceKind::Directory {
            return unavailable("CHANGED");
        }
        let identity = root_identity(root);
        if let Some(tree) = self.trees.get(path)
            && tree.root_identity == identity
            && !force
            && tree.checked.elapsed() < tree.interval
        {
            return tree.observed.clone();
        }
        // Bound memory for the shared desktop adapter; eviction is a safe cold scan.
        if !self.trees.contains_key(path) && self.trees.len() >= 32 {
            let oldest = self
                .trees
                .iter()
                .min_by_key(|(_, tree)| tree.checked)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                self.trees.remove(&oldest);
            }
        }
        let now = Instant::now();
        let tree = self.trees.entry(path.into()).or_insert_with(|| CachedTree {
            directories: HashMap::new(),
            observed: unavailable("UNKNOWN"),
            checked: now,
            reconciled: now,
            interval: Duration::ZERO,
            root_identity: identity.clone(),
        });
        let full = tree.root_identity != identity
            || tree.reconciled.elapsed() >= Duration::from_secs(60)
            || tree.observed.unknown_entries != 0;
        if full {
            tree.directories.clear();
            tree.reconciled = now;
        }
        let observed = scan(root, &mut tree.directories);
        tree.interval = now
            .elapsed()
            .saturating_mul(4)
            .clamp(Duration::from_secs(1), Duration::from_secs(30));
        tree.checked = Instant::now();
        tree.root_identity = identity;
        tree.observed = observed.clone();
        observed
    }
}

fn scan(root: &Path, directories: &mut HashMap<PathBuf, DirectoryListing>) -> ResourceMeasurement {
    let mut result = ResourceMeasurement {
        availability: "PRESENT".into(),
        size_bytes: Some(0),
        unknown_entries: 0,
        total_entries: 0,
        entries: Vec::new(),
    };
    let mut queue = VecDeque::from([(root.to_path_buf(), 0_usize)]);
    let mut visited = HashSet::new();
    let mut bytes = 0_u64;
    while let Some((directory, depth)) = queue.pop_front() {
        if depth > 64 || check_components(&directory).is_err() {
            result.unknown_entries += 1;
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&directory) else {
            result.unknown_entries += 1;
            continue;
        };
        if !metadata.is_dir() {
            result.unknown_entries += 1;
            continue;
        }
        let modified = metadata.modified().ok();
        visited.insert(directory.clone());
        let needs_listing = directories
            .get(&directory)
            .is_none_or(|cached| modified.is_none() || cached.modified != modified);
        if needs_listing {
            let children = fs::read_dir(&directory).and_then(|children| {
                let mut paths = Vec::new();
                for child in children {
                    if paths.len() >= 100_000 {
                        return Err(std::io::Error::other("Directory entry limit exceeded"));
                    }
                    paths.push(child?.path());
                }
                paths.sort();
                Ok(paths)
            });
            if let Ok(children) = children {
                directories.insert(directory.clone(), DirectoryListing { modified, children });
            } else {
                directories.remove(&directory);
                result.unknown_entries += 1;
                continue;
            }
        }
        for path in &directories[&directory].children {
            if result.total_entries >= 100_000 {
                result.unknown_entries += 1;
                queue.clear();
                break;
            }
            result.total_entries += 1;
            if check_components(path).is_err() {
                result.unknown_entries += 1;
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(path) else {
                result.unknown_entries += 1;
                continue;
            };
            let (kind, size) = if metadata.is_dir() {
                queue.push_back((path.clone(), depth + 1));
                ("DIRECTORY", None)
            } else if metadata.is_file() {
                bytes = bytes.saturating_add(metadata.len());
                ("FILE", Some(metadata.len()))
            } else {
                result.unknown_entries += 1;
                continue;
            };
            if result.entries.len() < 200 {
                result.entries.push(ResourceContentEntry {
                    relative_path: path
                        .strip_prefix(root)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .into_owned(),
                    kind: kind.into(),
                    size_bytes: size,
                });
            }
        }
        if fs::metadata(&directory)
            .ok()
            .and_then(|meta| meta.modified().ok())
            != modified
        {
            directories.remove(&directory);
            result.unknown_entries += 1;
        }
    }
    directories.retain(|path, _| visited.contains(path));
    result.size_bytes = Some(bytes);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_listings_detect_modified_and_removed_files_without_reading_contents() {
        let root = std::env::temp_dir().join(format!("casklet-metadata-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/data.txt"), b"one").unwrap();
        let path = root.to_string_lossy();
        let mut cache = MeasurementCache::default();
        assert_eq!(
            cache
                .measure(&path, ResourceKind::Directory, true)
                .size_bytes,
            Some(3)
        );
        fs::write(root.join("nested/data.txt"), b"modified bytes").unwrap();
        assert_eq!(
            cache
                .measure(&path, ResourceKind::Directory, true)
                .size_bytes,
            Some(14)
        );
        fs::remove_file(root.join("nested/data.txt")).unwrap();
        assert_eq!(
            cache
                .measure(&path, ResourceKind::Directory, true)
                .size_bytes,
            Some(0)
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn periodic_reconciliation_recovers_an_unobserved_directory_change() {
        let root = std::env::temp_dir().join(format!("casklet-fallback-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.to_string_lossy();
        let mut cache = MeasurementCache::default();
        cache.measure(&path, ResourceKind::Directory, true);
        fs::write(root.join("missed.txt"), b"new").unwrap();
        let tree = cache.trees.get_mut(path.as_ref()).unwrap();
        // Simulate a missed/unchanged directory timestamp without altering files.
        tree.directories.get_mut(&root).unwrap().modified =
            fs::metadata(&root).unwrap().modified().ok();
        assert_eq!(
            cache
                .measure(&path, ResourceKind::Directory, true)
                .size_bytes,
            Some(0)
        );
        cache.trees.get_mut(path.as_ref()).unwrap().reconciled =
            Instant::now().checked_sub(Duration::from_secs(61)).unwrap();
        let fresh = cache.measure(&path, ResourceKind::Directory, true);
        assert_eq!(fresh.size_bytes, Some(3));
        assert_eq!(fresh.total_entries, 1);
        fs::remove_dir_all(root).unwrap();
    }
}
