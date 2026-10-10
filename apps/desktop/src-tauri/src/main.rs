// Windows release 下不弹控制台
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![bridge::execute, bridge::list_commands])
        .run(tauri::generate_context!())
        .expect("error while running Yanmo");
}
