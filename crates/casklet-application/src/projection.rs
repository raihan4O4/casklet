//! Retryable per-Cask recovery manifests.
use crate::{CaskRepository, CaskService, api_error, resource::ResourceRepository};
use casklet_contracts::{ApiError, CategoryView, DetectedApplicationView, ResourceView};
use casklet_domain::Cask;

pub trait ProjectionRepository {
    /// # Errors
    /// Returns associated detected installations for a Cask manifest.
    fn projection_installed(&self, cask_id: &str) -> Result<Vec<DetectedApplicationView>, String>;
    /// # Errors
    /// Returns pending Cask IDs and their target revisions.
    fn projection_jobs(&self) -> Result<Vec<(String, i64)>, String>;
    /// # Errors
    /// Marks only the revision just projected complete.
    fn complete_projection(&self, cask_id: &str, revision: i64) -> Result<(), String>;
    /// # Errors
    /// Records a retryable, path-free error code.
    fn fail_projection(&self, cask_id: &str, code: &str) -> Result<(), String>;
}

pub trait ManifestProjection {
    /// # Errors
    /// Returns a filesystem or serialization error.
    fn project(
        &self,
        cask: &Cask,
        resources: &[ResourceView],
        categories: &[CategoryView],
        applications: &[DetectedApplicationView],
    ) -> Result<(), String>;
}

impl<R: CaskRepository + ResourceRepository + ProjectionRepository, S: ManifestProjection>
    CaskService<R, S>
{
    /// Retries every pending manifest without failing the whole batch for one Cask.
    /// # Errors
    /// Returns a persistence error.
    pub fn retry_manifest_projections(&self, correlation_id: &str) -> Result<usize, ApiError> {
        let jobs = self
            .repository
            .projection_jobs()
            .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
        let mut completed = 0;
        for (cask_id, revision) in jobs {
            let projected = (|| {
                let cask = self.repository.get(&cask_id)?.ok_or("Cask missing")?;
                let resources = self.repository.list_resources(&cask_id)?;
                let categories = self.repository.list_categories(&cask_id)?;
                let applications = self.repository.projection_installed(&cask_id)?;
                self.storage
                    .project(&cask, &resources, &categories, &applications)
            })();
            match projected {
                Ok(()) => {
                    self.repository
                        .complete_projection(&cask_id, revision)
                        .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
                    completed += 1;
                }
                Err(_) => {
                    self.repository
                        .fail_projection(&cask_id, "PROJECTION_FAILED")
                        .map_err(|e| api_error("PERSISTENCE", e, correlation_id))?;
                }
            }
        }
        Ok(completed)
    }
}
