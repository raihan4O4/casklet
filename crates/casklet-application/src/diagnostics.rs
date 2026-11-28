//! Redacted operation diagnostics.
use crate::{CaskService, api_error, check_version, operation::OperationRepository};
use casklet_contracts::{ApiError, CreateDiagnosticsBundle, DiagnosticEvent, DiagnosticsBundle};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait DiagnosticsRepository {
    /// # Errors
    /// Returns bounded newest events and whether older events were omitted.
    fn diagnostic_events(&self, limit: usize) -> Result<(Vec<DiagnosticEvent>, bool), String>;
}

pub trait DiagnosticsWriter {
    /// # Errors
    /// Creates a new diagnostic file without overwriting an existing one.
    fn write_bundle(&self, path: &str, bundle: &DiagnosticsBundle) -> Result<(), String>;
}

impl<R: OperationRepository + DiagnosticsRepository, S: DiagnosticsWriter> CaskService<R, S> {
    /// Writes a diagnostic JSON bundle containing codes and operation IDs, never source paths.
    /// # Errors
    /// Returns a version, persistence, clock, or output error.
    pub fn create_diagnostics_bundle(
        &self,
        request: &CreateDiagnosticsBundle,
        correlation_id: &str,
    ) -> Result<(), ApiError> {
        check_version(request.version, correlation_id)?;
        if request.output_path.trim().is_empty() {
            return Err(api_error(
                "INVALID_OUTPUT",
                "Output path is empty".into(),
                correlation_id,
            ));
        }
        let generated_at_ms = i64::try_from(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?
                .as_millis(),
        )
        .map_err(|e| api_error("CLOCK", e.to_string(), correlation_id))?;
        let operations = self
            .repository
            .list_operations(None)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        let pending_manifest_count = self
            .repository
            .pending_manifests()
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?
            .len();
        let (events, events_truncated) = self
            .repository
            .diagnostic_events(1000)
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        self.storage
            .write_bundle(
                &request.output_path,
                &DiagnosticsBundle {
                    schema_version: 1,
                    generated_at_ms,
                    operations,
                    pending_manifest_count,
                    events,
                    events_truncated,
                },
            )
            .map_err(|e| api_error("DIAGNOSTICS_OUTPUT", e, correlation_id))
    }
}
