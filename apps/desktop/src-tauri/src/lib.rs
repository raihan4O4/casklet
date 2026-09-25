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
            list_casks,
            get_cask,
            add_resource,
            list_resources,
            get_resource,
            create_category,
            list_categories,
            categorize_resource,
            storage_summary
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Casklet desktop");
}

type Service = casklet_application::CaskService<
    casklet_infrastructure::SqliteCasks,
    casklet_infrastructure::LocalStorage,
>;

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
        service.add_resource(&request, "desktop", &|done, total| {
            let _ = app.emit("resource.progress", (done, total));
        })
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
