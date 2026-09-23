#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Starts the placeholder desktop shell.
///
/// # Panics
///
/// Panics if Tauri cannot initialize or run the desktop window.
pub fn run() {
    let _ = casklet_application::linked_boundaries();
    let _ = casklet_contracts::CRATE_BOUNDARY;
    let _ = casklet_infrastructure::CRATE_BOUNDARY;
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Casklet desktop foundation");
}
