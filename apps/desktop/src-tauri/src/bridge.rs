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
    if id == "compile.run" {
        return compile(&state, params).await;
    }
    let mut ws = state.workspace.lock().map_err(|e| e.to_string())?;
    state
        .registry
        .execute(&mut ws, &id, params)
        .map_err(|e| e.to_string())
}

/// 编译可能持续数十秒：只在准备与收尾时短暂持锁，编译本身在后台线程进行，
/// 期间读写文件、检查等命令照常响应。行为与注册表中的 `compile.run` 一致。
async fn compile(state: &State<'_, AppState>, params: Value) -> Result<Value, String> {
    let engine = match params.get("engine") {
        Some(Value::Null) | None => None,
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| e.to_string())?),
    };
    let job = {
        let mut ws = state.workspace.lock().map_err(|e| e.to_string())?;
        ws.compile_job(engine).map_err(|e| e.to_string())?
    };
    let output = tauri::async_runtime::spawn_blocking(move || job.run())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let value = serde_json::to_value(&output).map_err(|e| e.to_string())?;
    state
        .workspace
        .lock()
        .map_err(|e| e.to_string())?
        .finish_compile(output);
    Ok(value)
}

#[tauri::command]
pub fn list_commands(state: State<'_, AppState>) -> Vec<CommandMeta> {
    state.registry.list().cloned().collect()
}

/// 启动参数中的项目文件夹（`yanmo-desktop <文件夹>`），供前端启动后自动打开。
#[tauri::command]
pub fn launch_folder() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-') && std::path::Path::new(a).is_dir())
}
