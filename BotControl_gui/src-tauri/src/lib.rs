use log::LevelFilter;

mod state_manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {

        // Initialize env_logger
    env_logger::builder()
        .filter_level(LevelFilter::Debug) // Set to Debug for development
        .init();

    log::info!("Starting Tauri app with logging enabled");

    
    tauri::Builder
        ::default()
        .manage(state_manager::AppState::new())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            state_manager::connect_udp,

        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
