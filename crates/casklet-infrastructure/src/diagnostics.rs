//! Path-free diagnostics snapshot and bundle writer.
use super::{LocalStorage, SqliteCasks};
use casklet_application::diagnostics::{DiagnosticsRepository, DiagnosticsWriter};
use casklet_contracts::{DiagnosticEvent, DiagnosticsBundle};
use std::{fs::OpenOptions, io::Write};

impl DiagnosticsRepository for SqliteCasks {
    fn diagnostic_events(&self, limit: usize) -> Result<(Vec<DiagnosticEvent>, bool), String> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT operation_id,event_code,at_ms FROM diagnostic_events ORDER BY id DESC LIMIT ?1")
            .map_err(|e|e.to_string())?;
        let mut items = statement
            .query_map(
                [i64::try_from(limit.saturating_add(1)).map_err(|e| e.to_string())?],
                |row| {
                    Ok(DiagnosticEvent {
                        operation_id: row.get(0)?,
                        code: row.get(1)?,
                        at_ms: row.get(2)?,
                    })
                },
            )
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        let truncated = items.len() > limit;
        items.truncate(limit);
        Ok((items, truncated))
    }
}

impl DiagnosticsWriter for LocalStorage {
    fn write_bundle(&self, path: &str, bundle: &DiagnosticsBundle) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(bundle).map_err(|e| e.to_string())?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        if let Err(error) = file.write_all(&bytes).and_then(|()| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(path);
            return Err(error.to_string());
        }
        Ok(())
    }
}
