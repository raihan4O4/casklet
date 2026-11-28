mod application_icons;
mod explorer;
mod managed_msi;
use tauri::{Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the desktop shell and shared Cask service.
///
/// # Panics
///
/// Panics if Tauri cannot initialize or run the desktop window.
#[expect(clippy::too_many_lines, reason = "Explicit desktop command registry")]
pub fn run() {
    #[cfg(windows)]
    if std::env::args().nth(1).as_deref() == Some("--register-explorer") {
        let result = std::env::current_exe()
            .map_err(|e| e.to_string())
            .and_then(|path| {
                let library = path
                    .with_file_name("explorer")
                    .join(env!("CASKLET_EXPLORER_LIBRARY"));
                casklet_infrastructure::explorer::register(&path, &library)
            });
        std::process::exit(i32::from(result.is_err()));
    }
    let mut context = tauri::generate_context!();
    if cfg!(debug_assertions) {
        context.config_mut().identifier.push_str(".debug");
    }
    let launch_args = std::env::args().collect::<Vec<_>>();
    tauri::Builder::default()
        .manage(explorer::Runtime::new(&launch_args))
        .manage(explorer::Requests::default())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            explorer::receive(app, &args);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(move |app| {
            let service = if cfg!(debug_assertions) {
                casklet_infrastructure::development_service()
            } else {
                casklet_infrastructure::production_service()
            };
            if !initialize_service(app, service) {
                return Ok(());
            }
            explorer::initialized(app.handle(), &launch_args);
            if let Some(main) = app.get_webview_window("main") {
                watch_main_window(&main);
            }
            Ok(())
        })
        .manage(std::sync::Mutex::new(()))
        .invoke_handler(tauri::generate_handler![
            application_icons::application_icons,
            explorer::explorer_pending,
            explorer::explorer_ack,
            explorer::explorer_route,
            create_cask,
            update_cask,
            cask_library,
            open_cask_window,
            window_ready,
            set_window_theme,
            open_cask_storage,
            open_storage_root,
            open_resource_location,
            export_cask,
            preview_recovery_manifest,
            recover_manifest,
            scan_recovery_manifests,
            recover_discovered_manifests,
            list_casks,
            get_cask,
            add_resource,
            adopt_resource,
            set_resource_ownership,
            set_cask_archived,
            remove_cask,
            cask_removal_status,
            managed_msi::claim_managed_msi,
            managed_msi::install_application,
            managed_msi::launch_managed_msi_artifact,
            managed_msi::create_msi_cleanup,
            managed_msi::create_application_removal,
            managed_msi::latest_msi_cleanup,
            managed_msi::approve_msi_cleanup,
            managed_msi::execute_msi_cleanup,
            managed_msi::close_msi_cleanup,
            managed_msi::release_managed_msi,
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
            resource_contents,
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
            backup_database,
            create_removal_plan,
            create_category_cleanup,
            get_category_cleanup,
            pending_whole_cask_cleanup,
            approve_category_cleanup,
            execute_category_cleanup,
            get_removal_plan,
            list_removal_plans,
            removal_capability,
            approve_removal_plan,
            execute_removal_plan,
            close_uncertain_removal
        ])
        .build(context)
        .expect("failed to build Casklet desktop")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event
                && app
                    .state::<explorer::Runtime>()
                    .jobs
                    .load(std::sync::atomic::Ordering::SeqCst)
                    > 0
            {
                api.prevent_exit();
            }
        });
}

fn watch_main_window(main: &tauri::WebviewWindow) {
    let app = main.app_handle().clone();
    main.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let runtime = app.state::<explorer::Runtime>();
            runtime.ui.store(false, std::sync::atomic::Ordering::SeqCst);
            runtime
                .ready
                .store(false, std::sync::atomic::Ordering::SeqCst);
        }
    });
}

fn ensure_main_window(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(main) = app.get_webview_window("main") {
        return Ok(main);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .ok_or("Main window configuration unavailable")?;
    let main = tauri::WebviewWindowBuilder::from_config(app, config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    watch_main_window(&main);
    Ok(main)
}

fn initialize_service(app: &mut tauri::App, service: Result<Service, String>) -> bool {
    match service {
        Ok(service) => {
            app.manage(service);
            true
        }
        Err(problem) => {
            app.dialog().message(format!("Casklet could not open its local data. Reopen newer data with the matching Casklet version, or restore a verified database backup. Resource files have not been removed.\n\nDetails: {problem}"))
                        .title("Casklet cannot open your data")
                        .kind(tauri_plugin_dialog::MessageDialogKind::Error).blocking_show();
            app.handle().exit(1);
            false
        }
    }
}

type Service = casklet_application::CaskService<
    casklet_infrastructure::SqliteCasks,
    casklet_infrastructure::LocalStorage,
>;

const DETAIL_WIDTH: f64 = 560.0;
const DETAIL_HEIGHT: f64 = 760.0;

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn open_cask_window(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    gate: tauri::State<'_, std::sync::Mutex<()>>,
    request: casklet_contracts::GetCask,
) -> Result<(), casklet_contracts::ApiError> {
    let cask = service.get(&request, "desktop")?;
    if !cask
        .id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(window_error("Invalid Cask window identity"));
    }
    let _guard = gate
        .lock()
        .map_err(|_| window_error("Window manager unavailable"))?;
    let label = format!("cask-{}", cask.id);
    if let Some(window) = app.get_webview_window(&label) {
        // A second click during initialization must not expose the hidden WebView.
        if !window
            .is_visible()
            .map_err(|e| window_error(e.to_string()))?
        {
            return Ok(());
        }
        window
            .unminimize()
            .map_err(|e| window_error(e.to_string()))?;
        window.show().map_err(|e| window_error(e.to_string()))?;
        return window.set_focus().map_err(|e| window_error(e.to_string()));
    }
    let anchor = app
        .get_webview_window("main")
        .or_else(|| app.webview_windows().into_values().next());
    let main = anchor.ok_or_else(|| window_error("Window unavailable"))?;
    let scale = main
        .scale_factor()
        .map_err(|e| window_error(e.to_string()))?;
    let position = main
        .inner_position()
        .map_err(|e| window_error(e.to_string()))?;
    let size = main.inner_size().map_err(|e| window_error(e.to_string()))?;
    let x = f64::from(position.x) / scale + (f64::from(size.width) / scale - DETAIL_WIDTH) / 2.0;
    let y = f64::from(position.y) / scale + (f64::from(size.height) / scale - DETAIL_HEIGHT) / 2.0;
    let theme = main.theme().unwrap_or(tauri::Theme::Light);
    let window = tauri::WebviewWindowBuilder::new(
        &app,
        label,
        tauri::WebviewUrl::App(format!("index.html?cask={}", cask.id).into()),
    )
    .title(format!("{} \u{2014} Casklet", cask.name))
    .inner_size(DETAIL_WIDTH, DETAIL_HEIGHT)
    .resizable(false)
    .decorations(false)
    .maximizable(false)
    .minimizable(true)
    .visible(false)
    .theme(Some(theme))
    .background_color(theme_background(theme))
    .position(x, y.max(0.0))
    // Independent top-level windows must not inherit Windows owner minimization.
    .build()
    .map_err(|e| window_error(e.to_string()))?;
    let closing_handle = app.clone();
    let closing_label = format!("cask-{}", cask.id);
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            explorer::window_closed(&closing_handle, &closing_label);
            if let Some(main) = closing_handle.get_webview_window("main") {
                let _ = main.emit("casks-changed", ());
            }
        }
    });
    Ok(())
}

/// The invoking `WebView` reveals only its own already-rendered native window.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn window_ready(
    window: tauri::WebviewWindow,
    runtime: tauri::State<'_, explorer::Runtime>,
) -> Result<(), casklet_contracts::ApiError> {
    if window.label() == "main" {
        runtime
            .ready
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if !runtime.ui.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(());
        }
    }
    window.show().map_err(|e| window_error(e.to_string()))?;
    window.set_focus().map_err(|e| window_error(e.to_string()))
}

// Appearance is shell presentation state, not Cask domain data.
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum DesktopTheme {
    Light,
    Dark,
}

const fn theme_background(theme: tauri::Theme) -> tauri::window::Color {
    match theme {
        tauri::Theme::Dark => tauri::window::Color(32, 34, 38, 255),
        _ => tauri::window::Color(255, 255, 255, 255),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn set_window_theme(
    window: tauri::WebviewWindow,
    theme: DesktopTheme,
) -> Result<(), casklet_contracts::ApiError> {
    let native = match theme {
        DesktopTheme::Light => tauri::Theme::Light,
        DesktopTheme::Dark => tauri::Theme::Dark,
    };
    window
        .set_theme(Some(native))
        .map_err(|e| window_error(e.to_string()))?;
    window
        .set_background_color(Some(theme_background(native)))
        .map_err(|e| window_error(e.to_string()))
}

fn window_error(message: impl Into<String>) -> casklet_contracts::ApiError {
    casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "WINDOW".into(),
        message: message.into(),
        correlation_id: "desktop".into(),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn cask_library(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListCasks,
) -> Result<casklet_contracts::CaskLibraryView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.cask_library(&request, "desktop"))
        .await
        .map_err(|e| window_error(e.to_string()))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn set_cask_archived(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::SetCaskArchived,
) -> Result<casklet_contracts::CaskView, casklet_contracts::ApiError> {
    let result = service.set_cask_archived(&request, "desktop")?;
    let _ = app.emit("casks-changed", &result.id);
    Ok(result)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn cask_removal_status(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetCask,
) -> Result<casklet_contracts::CaskRemovalStatus, casklet_contracts::ApiError> {
    service.cask_removal_status(&request, "desktop")
}
fn announce_cask_removed(app: &tauri::AppHandle, id: &str) {
    let _ = app.emit("casks-removed", id);
    let _ = app.emit("casks-changed", ());
    if let Some(window) = app.get_webview_window(&format!("cask-{id}")) {
        let _ = window.destroy();
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn remove_cask(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::RemoveCask,
) -> Result<Option<String>, casklet_contracts::ApiError> {
    let id = request.id.clone();
    let service = service.inner().clone();
    let warning =
        tauri::async_runtime::spawn_blocking(move || service.remove_cask(&request, "desktop"))
            .await
            .map_err(|e| worker_error(&e))??;
    announce_cask_removed(&app, &id);
    Ok(warning)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn adopt_resource(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::AdoptResource,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.adopt_resource(&request, "desktop", &|done, total| {
            let _ = app.emit("resource-progress", (done, total));
        })?;
        let _ = app.emit("casks-changed", &result.cask_id);
        Ok(result)
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn set_resource_ownership(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::SetResourceOwnership,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = service.set_resource_ownership(&request, "desktop")?;
        let _ = app.emit("casks-changed", &result.cask_id);
        Ok(result)
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn update_cask(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::UpdateCask,
) -> Result<casklet_contracts::CaskView, casklet_contracts::ApiError> {
    let result = service.update_cask(&request, "desktop")?;
    if let Some(window) = app.get_webview_window(&format!("cask-{}", result.id)) {
        let _ = window.set_title(&format!("{} \u{2014} Casklet", result.name));
    }
    let _ = app.emit("casks-changed", &result.id);
    Ok(result)
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn open_cask_storage(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetCask,
) -> Result<(), casklet_contracts::ApiError> {
    service.open_cask_storage(&request, "desktop")
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
fn open_storage_root(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListCasks,
) -> Result<(), casklet_contracts::ApiError> {
    service.open_storage_root(&request, "desktop")
}

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
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::LaunchInstallerArtifact,
) -> Result<casklet_contracts::InstallerOperationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.launch_installer_artifact(&request, "desktop", &|id| {
            let _ = window.emit_to(window.label(), "installer-operation-started", id);
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
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InstallWingetPackage,
) -> Result<casklet_contracts::WingetInstallationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.install_winget_package(&request, "desktop", &|id| {
            let _ = window.emit_to(window.label(), "winget-operation-started", id);
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
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ExecuteWingetUninstallPlan,
) -> Result<casklet_contracts::WingetUninstallPlanView, casklet_contracts::ApiError> {
    #[cfg(feature = "reviewed-removal")]
    {
        let service = service.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            service.execute_winget_uninstall_plan(&request, "desktop", &|id| {
                let _ = window.emit_to(window.label(), "winget-operation-started", id);
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
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::InstallPortableZip,
) -> Result<casklet_contracts::PortableInstallationView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.install_portable_zip(
            &request,
            "desktop",
            &|done, total| {
                let _ = window.emit_to(window.label(), "portable-progress", (done, total));
            },
            &|id| {
                let _ = window.emit_to(window.label(), "operation-started", id);
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
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::AddResource,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.add_user_resource_with_events(
            &request,
            "desktop",
            &|done, total| {
                let _ = window.emit_to(window.label(), "resource-progress", (done, total));
            },
            &|id| {
                let _ = window.emit_to(window.label(), "operation-started", id);
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
async fn list_resources(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListResources,
    refresh: Option<bool>,
) -> Result<Vec<casklet_contracts::ResourceView>, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if refresh.unwrap_or(false) {
            service.list_resources(&request, "desktop")
        } else {
            service.list_resource_metadata(&request, "desktop")
        }
    })
    .await
    .map_err(|e| window_error(e.to_string()))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn open_resource_location(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetResource,
) -> Result<(), casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.open_resource_location(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn get_resource(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetResource,
) -> Result<casklet_contracts::ResourceView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.get_resource(&request, "desktop"))
        .await
        .map_err(|e| window_error(e.to_string()))?
}
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn resource_contents(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetResource,
    refresh: Option<bool>,
) -> Result<casklet_contracts::ResourceContents, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        service.resource_contents(&request, refresh.unwrap_or(false), "desktop")
    })
    .await
    .map_err(|e| window_error(e.to_string()))?
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
async fn storage_summary(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetStorageSummary,
    refresh: Option<bool>,
) -> Result<casklet_contracts::StorageSummary, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if refresh.unwrap_or(false) {
            service.storage_summary(&request, "desktop")
        } else {
            service.storage_summary_metadata(&request, "desktop")
        }
    })
    .await
    .map_err(|e| window_error(e.to_string()))?
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
async fn backup_database(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::BackupDatabase,
) -> Result<casklet_contracts::DatabaseBackupView, casklet_contracts::ApiError> {
    let service = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || service.backup_database(&request, "desktop"))
        .await
        .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn create_removal_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateRemovalPlan,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    let worker = Service {
        repository: service.repository.clone(),
        storage: service.storage.clone(),
    };
    tauri::async_runtime::spawn_blocking(move || worker.create_removal_plan(&request, "desktop"))
        .await
        .map_err(|_| cleanup_worker_error())?
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
#[allow(clippy::needless_pass_by_value)]
fn list_removal_plans(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ListRemovalPlans,
) -> Result<Vec<casklet_contracts::RemovalPlanView>, casklet_contracts::ApiError> {
    service.list_removal_plans(&request, "desktop")
}

fn cleanup_worker_error() -> casklet_contracts::ApiError {
    casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "WORKER_INTERRUPTED".into(),
        message: "Cleanup worker stopped; inspect recovery before retrying".into(),
        correlation_id: "desktop".into(),
    }
}

#[tauri::command]
fn removal_capability() -> bool {
    cfg!(any(feature = "reviewed-removal", feature = "alpha-cleanup"))
}

fn removal_review_pending() -> casklet_contracts::ApiError {
    casklet_contracts::ApiError {
        version: casklet_contracts::CONTRACT_VERSION,
        code: "REMOVAL_REVIEW_PENDING".into(),
        message: "This destructive action is disabled until its security review is complete".into(),
        correlation_id: "desktop".into(),
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn approve_removal_plan(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ApproveRemovalPlan,
) -> Result<casklet_contracts::RemovalPlanView, casklet_contracts::ApiError> {
    #[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
    {
        let worker = Service {
            repository: service.repository.clone(),
            storage: service.storage.clone(),
        };
        tauri::async_runtime::spawn_blocking(move || {
            worker.approve_removal_plan(&request, "desktop")
        })
        .await
        .map_err(|_| cleanup_worker_error())?
    }
    #[cfg(not(any(feature = "reviewed-removal", feature = "alpha-cleanup")))]
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
    #[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
    {
        let worker = Service {
            repository: service.repository.clone(),
            storage: service.storage.clone(),
        };
        tauri::async_runtime::spawn_blocking(move || {
            worker.execute_removal_plan(&request, "desktop")
        })
        .await
        .map_err(|_| cleanup_worker_error())?
    }
    #[cfg(not(any(feature = "reviewed-removal", feature = "alpha-cleanup")))]
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
    #[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
    {
        service.close_uncertain_removal(&request, "desktop")
    }
    #[cfg(not(any(feature = "reviewed-removal", feature = "alpha-cleanup")))]
    {
        let _ = (service, request);
        Err(removal_review_pending())
    }
}

#[cfg(test)]
mod alpha_tests {
    #[test]
    fn file_cleanup_capability_matches_build_profile() {
        assert_eq!(
            super::removal_capability(),
            cfg!(any(feature = "alpha-cleanup", feature = "reviewed-removal"))
        );
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn create_category_cleanup(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CreateCategoryCleanup,
) -> Result<casklet_contracts::CategoryCleanupPlan, casklet_contracts::ApiError> {
    let worker = service.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        worker.create_category_cleanup(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?;
    let _ = app.emit("casks-changed", ());
    result
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn get_category_cleanup(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CategoryCleanupPlan,
) -> Result<casklet_contracts::CategoryCleanupPlan, casklet_contracts::ApiError> {
    let worker = service.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        worker.get_category_cleanup(&request, "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?;
    if let Ok(plan) = &result
        && plan.cask_removed
    {
        announce_cask_removed(&app, &plan.cask_id);
    } else {
        let _ = app.emit("casks-changed", ());
    }
    result
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn pending_whole_cask_cleanup(
    service: tauri::State<'_, Service>,
    request: casklet_contracts::GetCask,
    resource_plan_id: Option<String>,
) -> Result<Option<casklet_contracts::CategoryCleanupPlan>, casklet_contracts::ApiError> {
    let worker = service.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker.pending_whole_cask_cleanup(&request, resource_plan_id.as_deref(), "desktop")
    })
    .await
    .map_err(|e| worker_error(&e))?
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn approve_category_cleanup(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::CategoryCleanupPlan,
) -> Result<casklet_contracts::CategoryCleanupPlan, casklet_contracts::ApiError> {
    #[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
    {
        let worker = service.inner().clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            worker.approve_category_cleanup(&request, "desktop")
        })
        .await
        .map_err(|e| worker_error(&e))?;
        let _ = app.emit("casks-changed", ());
        result
    }
    #[cfg(not(any(feature = "reviewed-removal", feature = "alpha-cleanup")))]
    {
        let _ = (app, service, request);
        Err(removal_review_pending())
    }
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
async fn execute_category_cleanup(
    app: tauri::AppHandle,
    service: tauri::State<'_, Service>,
    request: casklet_contracts::ExecuteCategoryCleanup,
) -> Result<casklet_contracts::CategoryCleanupPlan, casklet_contracts::ApiError> {
    #[cfg(any(feature = "reviewed-removal", feature = "alpha-cleanup"))]
    {
        let worker = service.inner().clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            worker.execute_category_cleanup(&request, "desktop")
        })
        .await
        .map_err(|e| worker_error(&e))?;
        if let Ok(plan) = &result
            && plan.cask_removed
        {
            announce_cask_removed(&app, &plan.cask_id);
        } else {
            let _ = app.emit("casks-changed", ());
        }
        result
    }
    #[cfg(not(any(feature = "reviewed-removal", feature = "alpha-cleanup")))]
    {
        let _ = (app, service, request);
        Err(removal_review_pending())
    }
}
