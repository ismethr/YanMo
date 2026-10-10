//! 前端与 Rust 之间的唯一桥：前端只调用 `execute(id, params)`，
//! 不为每个功能单独开 Tauri command，保证与 CLI / MCP 行为一致。

use serde_json::Value;
use std::sync::Mutex;
use tauri::State;
use ym_commands::{CommandMeta, Registry, Workspace};

pub struct AppState {
    registry: Registry,
    workspace: Mutex<Workspace>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            registry: Registry::with_builtins(),
            workspace: Mutex::new(Workspace::default()),
        }
    }
}

#[tauri::command]
pub async fn execute(
    state: State<'_, AppState>,
    id: String,
    params: Value,
) -> Result<Value, String> {
    let mut ws = state.workspace.lock().map_err(|e| e.to_string())?;
    state
        .registry
        .execute(&mut ws, &id, params)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_commands(state: State<'_, AppState>) -> Vec<CommandMeta> {
    state.registry.list().cloned().collect()
}
