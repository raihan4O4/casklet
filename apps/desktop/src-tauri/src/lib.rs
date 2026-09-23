#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = casklet_application::linked_boundaries();
    let _ = casklet_contracts::CRATE_BOUNDARY;
    let _ = casklet_infrastructure::CRATE_BOUNDARY;
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Casklet desktop foundation");
}
