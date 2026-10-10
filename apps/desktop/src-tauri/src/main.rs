// Windows release 下不弹控制台
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod bridge;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(bridge::AppState::default())
        .manage(ai::AiState::default())
        .invoke_handler(tauri::generate_handler![
            bridge::execute,
            bridge::list_commands,
            bridge::launch_folder,
            ai::ai_settings_get,
            ai::ai_settings_set,
            ai::ai_set_key,
            ai::ai_stream,
            ai::ai_cancel
        ])
        .run(tauri::generate_context!())
        .expect("error while running YanMo");
}
