//! Typed desktop adapters for managed MSI ownership and cleanup.
use super::{Service, removal_review_pending, worker_error};
use tauri::Emitter;

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn install_application(
    app: tauri::AppHandle,
    window: tauri::Window,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InstallApplication,
) -> Result<casklet_contracts::InstallerOperationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        service.install_application(&request, "desktop", &|id| {
            let _ = window.emit_to(window.label(), "installer.operation.started", id);
        })
    })
    .await
    .map_err(|e| worker_error(&e))?;
    let _ = app.emit("casks.changed", ());
    result
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn launch_managed_msi_artifact(
    window: tauri::Window,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::LaunchInstallerArtifact,
) -> Result<casklet_contracts::InstallerOperationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.launch_managed_msi_artifact(&request, "desktop", &|id| {
            let _ = window.emit_to(window.label(), "installer.operation.started", id);
        })
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn claim_managed_msi(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ConfirmInstallerCandidate,
) -> Result<(), casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.claim_managed_msi(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn create_msi_cleanup(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetCask,
) -> Result<casklet_contracts::MsiCleanupPlan, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.create_msi_cleanup(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) fn latest_msi_cleanup(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetCask,
) -> Result<Option<casklet_contracts::MsiCleanupPlan>, casklet_contracts::ApiError> {
    service.latest_msi_cleanup(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn approve_msi_cleanup(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ApproveRemovalPlan,
) -> Result<casklet_contracts::MsiCleanupPlan, casklet_contracts::ApiError> {
    if !cfg!(any(feature = "alpha-cleanup", feature = "reviewed-removal")) {
        return Err(removal_review_pending());
    }
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.approve_msi_cleanup(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn execute_msi_cleanup(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ApproveRemovalPlan,
) -> Result<casklet_contracts::MsiCleanupPlan, casklet_contracts::ApiError> {
    if !cfg!(any(feature = "alpha-cleanup", feature = "reviewed-removal")) {
        return Err(removal_review_pending());
    }
    let service = service.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        service.execute_msi_cleanup(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?;
    let _ = app.emit("casks.changed", ());
    result
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) async fn close_msi_cleanup(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetRemovalPlan,
) -> Result<casklet_contracts::MsiCleanupPlan, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.close_msi_cleanup(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) fn release_managed_msi(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::DisassociateInstalledApplication,
) -> Result<(), casklet_contracts::ApiError> {
    service.release_managed_msi(&request, "desktop")
}
