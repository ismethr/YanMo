//! 前端与 Rust 之间的唯一桥：前端只调用 `execute(id, params)`，
//! 不为每个功能单独开 Tauri command，保证与 CLI / MCP 行为一致。

use serde_json::Value;

#[tauri::command]
pub async fn execute(id: String, params: Value) -> Result<Value, String> {
    let _ = (id, params);
    Err("not implemented".into()) // TODO(M0): 接入 ym_commands::Registry
}

#[tauri::command]
pub fn list_commands() -> Vec<Value> {
    Vec::new() // TODO(M0)
}
