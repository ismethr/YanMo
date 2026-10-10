//! AI 命令。网络请求在 Rust 端发出，API Key 只在这里从钥匙串读取，界面永远拿不到明文。
//! 流式增量通过 `ai-event` 事件推送给界面（按请求 id 区分）。

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use ym_ai::prompts::{self, Task, TaskContext};
use ym_ai::settings::{get_key, set_key};
use ym_ai::{AiSettings, ProviderKind, StreamEvent};

#[derive(Default)]
pub struct AiState {
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Serialize, Clone)]
struct EventPayload {
    id: String,
    event: StreamEvent,
}

#[tauri::command]
pub fn ai_settings_get() -> Value {
    json!({
        "settings": AiSettings::load(),
        // 只告诉界面有没有 Key，不返回 Key 本身
        "has_key": {
            "anthropic": get_key(ProviderKind::Anthropic).is_some(),
            "openai_compatible": get_key(ProviderKind::OpenaiCompatible).is_some(),
        }
    })
}

#[tauri::command]
pub fn ai_settings_set(settings: AiSettings) -> Result<(), String> {
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_set_key(provider: ProviderKind, key: String) -> Result<(), String> {
    set_key(provider, &key).map_err(|e| e.to_string())
}

/// 执行一次 AI 任务。返回完整结果；改写任务额外返回提取出的 `rewrite`。
#[tauri::command]
pub async fn ai_stream(
    app: AppHandle,
    state: State<'_, AiState>,
    id: String,
    task: Task,
    context: TaskContext,
) -> Result<Value, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .running
        .lock()
        .map_err(|e| e.to_string())?
        .insert(id.clone(), cancel.clone());
    let request = prompts::build(&task, &context);
    let settings = AiSettings::load();
    let event_id = id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        ym_ai::stream(&settings, &request, &cancel, &mut |event| {
            let _ = app.emit(
                "ai-event",
                EventPayload {
                    id: event_id.clone(),
                    event,
                },
            );
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    state.running.lock().map_err(|e| e.to_string())?.remove(&id);
    let outcome = result.map_err(|e| e.to_string())?;
    let rewrite =
        matches!(task, Task::Rewrite { .. }).then(|| prompts::extract_rewrite(&outcome.text));
    Ok(json!({ "outcome": outcome, "rewrite": rewrite }))
}

#[tauri::command]
pub fn ai_cancel(state: State<'_, AiState>, id: String) {
    if let Ok(running) = state.running.lock() {
        if let Some(flag) = running.get(&id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
}
