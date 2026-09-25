//! Recovery manifest generation from committed SQLite state.
use super::{LocalStorage, SqliteCasks, resource_storage::check_components};
use casklet_application::projection::{ManifestProjection, ProjectionRepository};
use casklet_contracts::{CategoryView, ResourceView};
use casklet_domain::Cask;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use uuid::Uuid;

fn restore_previous_manifest(metadata: &Path, cask_id: &str) -> Result<(), String> {
    let destination = metadata.join("manifest.json");
    if destination.exists() {
        return Ok(());
    }
    let mut candidates = fs::read_dir(metadata)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with("manifest.")
                && Path::new(&name)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("prev"))
                && !name.contains("sha256")
        })
        .collect::<Vec<_>>();
    candidates.sort_by_key(|entry| {
        entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
    });
    for candidate in candidates.into_iter().rev() {
        let path = candidate.path();
        if check_components(&path).is_err() {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        if value["schemaVersion"] == 1 && value["cask"]["id"] == cask_id {
            fs::rename(path, &destination).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    Ok(())
}

impl ProjectionRepository for SqliteCasks {
    fn projection_jobs(&self) -> Result<Vec<(String, i64)>, String> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT cask_id,target_revision FROM projection_jobs WHERE state='PENDING' ORDER BY updated_at_ms")
            .map_err(|e| e.to_string())?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    fn complete_projection(&self, cask_id: &str, revision: i64) -> Result<(), String> {
        self.connection()?.execute("UPDATE projection_jobs SET state='COMPLETE',last_error_code=NULL WHERE cask_id=?1 AND target_revision=?2 AND state='PENDING'",params![cask_id,revision]).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn fail_projection(&self, cask_id: &str, code: &str) -> Result<(), String> {
        self.connection()?.execute("UPDATE projection_jobs SET attempt_count=attempt_count+1,last_error_code=?2 WHERE cask_id=?1 AND state='PENDING'",params![cask_id,code]).map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl ManifestProjection for LocalStorage {
    fn project(
        &self,
        cask: &Cask,
        resources: &[ResourceView],
        categories: &[CategoryView],
    ) -> Result<(), String> {
        let metadata = Path::new(&cask.managed_storage_root).join(".casklet");
        check_components(&metadata)?;
        fs::create_dir_all(&metadata).map_err(|e| e.to_string())?;
        check_components(&metadata)?;
        restore_previous_manifest(&metadata, &cask.id)?;
        let manifest = serde_json::json!({
            "schemaVersion":1,
            "cask":{"id":cask.id,"name":cask.name,"state":cask.state.code(),"createdAtMs":cask.created_at_ms},
            "resources":resources.iter().map(|item| serde_json::json!({
                "id":item.id,"name":item.name,"kind":item.kind,"management":item.management,
                "path":item.path,"containment":item.containment,"categoryKey":item.category_key,
                "sizeBytes":item.size_bytes,"sizeMeasurement":item.size_measurement
            })).collect::<Vec<_>>(),
            "categories":categories.iter().map(|item| serde_json::json!({
                "key":item.key,"name":item.name,"isBuiltin":item.is_builtin
            })).collect::<Vec<_>>(),
            "applications":[]
        });
        let bytes = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
        let token = Uuid::new_v4();
        let temp = metadata.join(format!("manifest.{token}.tmp"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let readback = fs::read(&temp).map_err(|e| e.to_string())?;
        serde_json::from_slice::<serde_json::Value>(&readback).map_err(|e| e.to_string())?;
        if readback != bytes {
            return Err("Manifest staging verification failed".into());
        }
        let destination = metadata.join("manifest.json");
        let backup = metadata.join(format!("manifest.{token}.prev"));
        let previous = destination.exists();
        if previous {
            fs::rename(&destination, &backup).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&temp, &destination) {
            if previous {
                let _ = fs::rename(&backup, &destination);
            }
            return Err(error.to_string());
        }
        let hash = format!("{:x}", Sha256::digest(&bytes));
        let hash_temp = metadata.join(format!("manifest.{token}.sha256.tmp"));
        let mut hash_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&hash_temp)
            .map_err(|e| e.to_string())?;
        hash_file
            .write_all(hash.as_bytes())
            .map_err(|e| e.to_string())?;
        hash_file.sync_all().map_err(|e| e.to_string())?;
        drop(hash_file);
        let hash_dest = metadata.join("manifest.sha256");
        let old_hash = metadata.join(format!("manifest.{token}.sha256.prev"));
        let had_hash = hash_dest.exists();
        if had_hash {
            fs::rename(&hash_dest, &old_hash).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&hash_temp, &hash_dest) {
            if had_hash {
                let _ = fs::rename(&old_hash, &hash_dest);
            }
            return Err(error.to_string());
        }
        if previous {
            let _ = fs::remove_file(backup);
        }
        if had_hash {
            let _ = fs::remove_file(old_hash);
        }
        Ok(())
    }
}
