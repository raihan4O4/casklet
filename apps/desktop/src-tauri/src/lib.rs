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
        .manage(service)
        .invoke_handler(tauri::generate_handler![create_cask, list_casks, get_cask])
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
