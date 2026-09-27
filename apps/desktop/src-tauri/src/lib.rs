use tauri::Emitter;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the desktop shell and shared Cask service.
///
/// # Panics
///
/// Panics if Tauri cannot initialize or run the desktop window.
pub fn run() {
    let service = casklet_infrastructure::development_service()
        .expect("failed to open Casklet development state");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(service)
        .invoke_handler(tauri::generate_handler![
            create_cask,
            export_cask,
            preview_recovery_manifest,
            recover_manifest,
            scan_recovery_manifests,
            recover_discovered_manifests,
            list_casks,
            get_cask,
            add_resource,
            inspect_portable_zip,
            install_portable_zip,
            list_portable_installations,
            get_portable_installation,
            launch_portable_installation,
            search_installed_applications,
            refresh_installed_applications,
            associate_installed_application,
            disassociate_installed_application,
            list_associated_applications,
            winget_capabilities,
            search_winget,
            review_winget_package,
            install_winget_package,
            list_winget_installations,
            get_winget_operation,
            cancel_winget_operation,
            close_winget_operation,
            create_winget_uninstall_plan,
            approve_winget_uninstall_plan,
            execute_winget_uninstall_plan,
            inspect_installer_artifact,
            register_installer_artifact,
            list_installer_artifacts,
            launch_installer_artifact,
            list_installer_operations,
            get_installer_operation,
            confirm_installer_candidate,
            cancel_installer_operation,
            close_installer_operation,
            list_resources,
            get_resource,
            create_category,
            list_categories,
            categorize_resource,
            storage_summary,
            list_operations,
            get_operation,
            recover_operation,
            preserve_operation,
            cancel_operation,
            recovery_status,
            inspect_recovery,
            retry_manifests,
            create_diagnostics_bundle,
            create_removal_plan,
            get_removal_plan,
            removal_capability,
            approve_removal_plan,
            execute_removal_plan,
            close_uncertain_removal
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Casklet desktop");
}

type Service = casklet_application::CaskService<
    casklet_infrastructure::SqliteCasks,
    casklet_infrastructure::LocalStorage,
>;

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn inspect_installer_artifact(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InspectInstallerArtifact,
) -> Result<casklet_contracts::InstallerInspection, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.inspect_installer_artifact(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn register_installer_artifact(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::RegisterInstallerArtifact,
) -> Result<casklet_contracts::InstallerArtifactView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.register_installer_artifact(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_installer_artifacts(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListInstallerArtifacts,
) -> Result<Vec<casklet_contracts::InstallerArtifactView>, casklet_contracts::ApiError> {
    service.list_installer_artifacts(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn launch_installer_artifact(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::LaunchInstallerArtifact,
) -> Result<casklet_contracts::InstallerOperationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.launch_installer_artifact(&request, "desktop", &|id| {
            let _ = app.emit("installer.operation.started", id);
        })
    })
    .await
    .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_installer_operations(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListInstallerOperations,
) -> Result<Vec<casklet_contracts::InstallerOperationView>, casklet_contracts::ApiError> {
    service.list_installer_operations(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_installer_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetInstallerOperation,
) -> Result<casklet_contracts::InstallerOperationView, casklet_contracts::ApiError> {
    service.get_installer_operation(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn confirm_installer_candidate(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ConfirmInstallerCandidate,
) -> Result<casklet_contracts::InstallerOperationView, casklet_contracts::ApiError> {
    service.confirm_installer_candidate(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn cancel_installer_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CancelInstallerOperation,
) -> Result<(), casklet_contracts::ApiError> {
    service.cancel_installer_operation(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn close_installer_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CloseInstallerOperation,
) -> Result<(), casklet_contracts::ApiError> {
    service.close_installer_operation(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn winget_capabilities(
    service: tauri::State<'_, Service>,
) -> casklet_contracts::WingetCapabilities {
    service.winget_capabilities()
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn search_winget(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::SearchWinget,
) -> Result<casklet_contracts::WingetSearchResult, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.search_winget(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn review_winget_package(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ReviewWingetPackage,
) -> Result<casklet_contracts::WingetPackageReview, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.review_winget_package(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn install_winget_package(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InstallWingetPackage,
) -> Result<casklet_contracts::WingetInstallationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.install_winget_package(&request, "desktop", &|id| {
            let _ = app.emit("winget.operation.started", id);
        })
    })
    .await
    .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_winget_installations(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListWingetInstallations,
) -> Result<Vec<casklet_contracts::WingetInstallationView>, casklet_contracts::ApiError> {
    service.list_winget_installations(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_winget_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetWingetOperation,
) -> Result<casklet_contracts::WingetOperationView, casklet_contracts::ApiError> {
    service.get_winget_operation(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn cancel_winget_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CancelWingetOperation,
) -> Result<(), casklet_contracts::ApiError> {
    service.cancel_winget_operation(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn close_winget_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CloseWingetOperation,
) -> Result<(), casklet_contracts::ApiError> {
    service.close_winget_operation(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn create_winget_uninstall_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateWingetUninstallPlan,
) -> Result<casklet_contracts::WingetUninstallPlanView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.create_winget_uninstall_plan(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn approve_winget_uninstall_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ApproveWingetUninstallPlan,
) -> Result<casklet_contracts::WingetUninstallPlanView, casklet_contracts::ApiError> {
    service.approve_winget_uninstall_plan(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value, unused_variables)]
async fn execute_winget_uninstall_plan(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ExecuteWingetUninstallPlan,
) -> Result<casklet_contracts::WingetUninstallPlanView, casklet_contracts::ApiError> {
    #[cfg(feature = "reviewed-removal")]
    {
        let service = service.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            service.execute_winget_uninstall_plan(&request, "desktop", &|id| {
                let _ = app.emit("winget.operation.started", id);
            })
        })
        .await
        .map_err(|e| worker_error(&e))?
    }
    #[cfg(not(feature = "reviewed-removal"))]
    {
        Err(removal_review_pending())
    }
}

fn worker_error(error: &impl ToString) -> casklet_contracts::ApiError {
    casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "WORKER".into(),
        message: error.to_string(),
        correlation_id: "desktop".into(),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn search_installed_applications(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::SearchInstalledApplications,
) -> Result<Vec<casklet_contracts::DetectedApplicationView>, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.search_installed_applications(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn refresh_installed_applications(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::RefreshInstalledApplications,
) -> Result<usize, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.refresh_installed_applications(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn associate_installed_application(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::AssociateInstalledApplication,
) -> Result<casklet_contracts::DetectedApplicationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.associate_installed_application(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn disassociate_installed_application(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::DisassociateInstalledApplication,
) -> Result<(), casklet_contracts::ApiError> {
    service.disassociate_installed_application(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_associated_applications(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListAssociatedApplications,
) -> Result<Vec<casklet_contracts::DetectedApplicationView>, casklet_contracts::ApiError> {
    service.list_associated_applications(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn inspect_portable_zip(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InspectPortableZip,
) -> Result<casklet_contracts::PortableZipPreview, casklet_contracts::ApiError> {
    service.inspect_portable_zip(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn install_portable_zip(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InstallPortableZip,
) -> Result<casklet_contracts::PortableInstallationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.install_portable_zip(
            &request,
            "desktop",
            &|done, total| {
                let _ = app.emit("portable.progress", (done, total));
            },
            &|id| {
                let _ = app.emit("operation.started", id);
            },
        )
    })
    .await
    .map_err(|e| casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "WORKER".into(),
        message: e.to_string(),
        correlation_id: "desktop".into(),
    })?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_portable_installations(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListPortableInstallations,
) -> Result<Vec<casklet_contracts::PortableInstallationView>, casklet_contracts::ApiError> {
    service.list_portable_installations(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_portable_installation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetPortableInstallation,
) -> Result<casklet_contracts::PortableInstallationView, casklet_contracts::ApiError> {
    service.get_portable_installation(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn launch_portable_installation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::LaunchPortableInstallation,
) -> Result<casklet_contracts::PortableInstallationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.launch_portable_installation(&request, "desktop")
    })
    .await
    .map_err(|e| casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "WORKER".into(),
        message: e.to_string(),
        correlation_id: "desktop".into(),
    })?
}

fn inventory_error(message: String) -> casklet_contracts::ApiError {
    casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "INVENTORY".into(),
        message,
        correlation_id: "desktop".into(),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn export_cask(
    service: tauri::State<'_, Service>,
    cask_id: String,
    output_path: String,
    format: String,
    machine: bool,
) -> Result<(), casklet_contracts::ApiError> {
    use casklet_infrastructure::inventory::{ExportProfile, write_export};
    let profile = if machine {
        ExportProfile::Machine
    } else {
        ExportProfile::Portable
    };
    let bytes = match format.as_str() {
        "json" => service
            .repository
            .export_inventory(&cask_id, profile)
            .and_then(|value| serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())),
        "markdown" => service
            .repository
            .export_report(&cask_id, profile)
            .map(String::into_bytes),
        _ => Err("Unsupported export format".into()),
    }
    .map_err(inventory_error)?;
    write_export(std::path::Path::new(&output_path), &bytes).map_err(inventory_error)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn preview_recovery_manifest(path: String) -> Result<String, casklet_contracts::ApiError> {
    casklet_infrastructure::inventory::preview_manifest(std::path::Path::new(&path))
        .map(|preview| {
            format!(
                "{}: {} resources ({} present, {} missing, {} unverified). {}",
                preview.cask_name,
                preview.resource_count,
                preview.observed_present,
                preview.observed_missing,
                preview.unverified,
                preview.warnings.join(" ")
            )
        })
        .map_err(inventory_error)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn recover_manifest(
    service: tauri::State<'_, Service>,
    path: String,
) -> Result<String, casklet_contracts::ApiError> {
    service
        .repository
        .recover_manifest(&service.storage, std::path::Path::new(&path))
        .map(|preview| format!("Recovered {} as unverified metadata.", preview.cask_name))
        .map_err(inventory_error)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn scan_recovery_manifests(
    service: tauri::State<'_, Service>,
) -> Result<String, casklet_contracts::ApiError> {
    casklet_infrastructure::inventory::discover_manifests(&service.storage)
        .map(|items| casklet_infrastructure::inventory::describe_manifests(&items))
        .map_err(inventory_error)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn recover_discovered_manifests(
    service: tauri::State<'_, Service>,
) -> Result<String, casklet_contracts::ApiError> {
    service
        .repository
        .recover_discovered(&service.storage)
        .map(|items| format!("Recovered {} Casks as unverified metadata.", items.len()))
        .map_err(inventory_error)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn create_cask(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateCask,
) -> Result<casklet_contracts::CaskView, casklet_contracts::ApiError> {
    service.create(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn list_casks(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListCasks,
) -> Result<Vec<casklet_contracts::CaskView>, casklet_contracts::ApiError> {
    service.list(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri command extractor requires owned arguments.
fn get_cask(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetCask,
) -> Result<casklet_contracts::CaskView, casklet_contracts::ApiError> {
    service.get(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn add_resource(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::AddResource,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.add_resource_with_events(
            &request,
            "desktop",
            &|done, total| {
                let _ = app.emit("resource.progress", (done, total));
            },
            &|id| {
                let _ = app.emit("operation.started", id);
            },
        )
    })
    .await
    .map_err(|e| casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "WORKER".into(),
        message: e.to_string(),
        correlation_id: "desktop".into(),
    })?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_resources(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListResources,
) -> Result<Vec<casklet_contracts::ResourceView>, casklet_contracts::ApiError> {
    service.list_resources(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_resource(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetResource,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    service.get_resource(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_category(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateCategory,
) -> Result<casklet_contracts::CategoryView, casklet_contracts::ApiError> {
    service.create_category(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_categories(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListCategories,
) -> Result<Vec<casklet_contracts::CategoryView>, casklet_contracts::ApiError> {
    service.list_categories(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn categorize_resource(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CategorizeResource,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    service.categorize_resource(&request, "desktop")
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn storage_summary(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetStorageSummary,
) -> Result<casklet_contracts::StorageSummary, casklet_contracts::ApiError> {
    service.storage_summary(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn list_operations(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListOperations,
) -> Result<Vec<casklet_contracts::OperationView>, casklet_contracts::ApiError> {
    service.list_operations(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetOperation,
) -> Result<casklet_contracts::OperationView, casklet_contracts::ApiError> {
    service.get_operation(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn recover_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::RecoverOperation,
) -> Result<casklet_contracts::OperationView, casklet_contracts::ApiError> {
    service.recover_operation(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn preserve_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::PreserveOperation,
) -> Result<casklet_contracts::OperationView, casklet_contracts::ApiError> {
    service.preserve_operation(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn cancel_operation(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CancelOperation,
) -> Result<casklet_contracts::OperationView, casklet_contracts::ApiError> {
    service.cancel_operation(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn recovery_status(
    service: tauri::State<'_, Service>,
) -> Result<casklet_contracts::RecoveryStatus, casklet_contracts::ApiError> {
    service.recovery_status("desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn inspect_recovery(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetOperation,
) -> Result<casklet_contracts::RecoveryInspection, casklet_contracts::ApiError> {
    service.inspect_recovery(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn retry_manifests(
    service: tauri::State<'_, Service>,
) -> Result<usize, casklet_contracts::ApiError> {
    service.retry_manifest_projections("desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_diagnostics_bundle(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateDiagnosticsBundle,
) -> Result<(), casklet_contracts::ApiError> {
    service.create_diagnostics_bundle(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn create_removal_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateRemovalPlan,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    service.create_removal_plan(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn get_removal_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetRemovalPlan,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    service.get_removal_plan(&request, "desktop")
}

#[tauri::command]
fn removal_capability() -> bool {
    cfg!(feature = "reviewed-removal")
}

#[cfg(not(feature = "reviewed-removal"))]
fn removal_review_pending() -> casklet_contracts::ApiError {
    casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "REMOVAL_REVIEW_PENDING".into(),
        message: "Cleanup is disabled until its security review is complete".into(),
        correlation_id: "desktop".into(),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn approve_removal_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ApproveRemovalPlan,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    #[cfg(feature = "reviewed-removal")]
    {
        service.approve_removal_plan(&request, "desktop")
    }
    #[cfg(not(feature = "reviewed-removal"))]
    {
        let _ = (service, request);
        Err(removal_review_pending())
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn execute_removal_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ExecuteRemovalPlan,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    #[cfg(feature = "reviewed-removal")]
    {
        let worker = Service {
            repository: service.repository.clone(),
            storage: service.storage.clone(),
        };
        tauri::async_runtime::spawn_blocking(move || {
            worker.execute_removal_plan(&request, "desktop")
        })
        .await
        .map_err(|_| casklet_contracts::ApiError {
            version: casklet_contracts::CONTRACT_VERSION,
            code: "WORKER_INTERRUPTED".into(),
            message: "Cleanup worker stopped; inspect recovery before retrying".into(),
            correlation_id: "desktop".into(),
        })?
    }
    #[cfg(not(feature = "reviewed-removal"))]
    {
        let _ = (service, request);
        Err(removal_review_pending())
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn close_uncertain_removal(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CloseRemovalRun,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    #[cfg(feature = "reviewed-removal")]
    {
        service.close_uncertain_removal(&request, "desktop")
    }
    #[cfg(not(feature = "reviewed-removal"))]
    {
        let _ = (service, request);
        Err(removal_review_pending())
    }
}
