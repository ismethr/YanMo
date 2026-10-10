//! 流式调用。Rust 没有官方 Anthropic SDK，这里直接使用 Messages 接口（SSE）。
//!
//! - Anthropic：`POST https://api.anthropic.com/v1/messages`，`stream: true`；Opus 5.5 等模型默认开启
//!   服务端回退（`fallbacks: "default"`，beta `server-side-fallback-2026-07-01`）——安全分类器误拒时
//!   在同一次请求内换模型继续，不用自己重试。仍然拒绝时返回 `stop_reason: "refusal"`。
//! - OpenAI 兼容：`POST {base}/chat/completions`，`stream: true`。

use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::settings::{get_key, AiSettings, ProviderKind};
use crate::{AiError, ChatRequest, Role};
use ym_model::tr;

/// 默认接口地址；可用环境变量 `ANTHROPIC_BASE_URL` 覆盖（与官方 SDK 的约定一致，便于走代理）
const ANTHROPIC_BASE: &str = "https://api.anthropic.com";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// 流式过程中的事件。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StreamEvent {
    /// 新增的正文
    Text(String),
    /// 提示信息（如：已切换到其他模型继续）
    Notice(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatOutcome {
    pub text: String,
    /// 实际作答的模型（发生回退时与设置不同）
    pub model: String,
    pub stop_reason: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        // 思考期间可能较长时间没有正文；服务端会定期发送 ping
        .timeout_read(Duration::from_secs(300))
        .user_agent(concat!("YanMo/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// 只有支持服务端回退的模型才带上 `fallbacks`（Haiku 等不支持，带上会报错或无效）。
fn supports_fallback(model: &str) -> bool {
    ["claude-opus-5", "claude-fable-5", "claude-sonnet-5-5"]
        .iter()
        .any(|p| model.starts_with(p))
}

fn http_error(e: ureq::Error) -> AiError {
    match e {
        ureq::Error::Status(status, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let message = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| {
                    v.pointer("/error/message")
                        .or_else(|| v.get("message"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or(body);
            AiError::Http { status, message }
        }
        other => AiError::Network(other.to_string()),
    }
}

/// 按设置调用对应服务商。`on_event` 收到增量；`cancel` 置位后尽快中止。
pub fn stream(
    settings: &AiSettings,
    req: &ChatRequest,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(StreamEvent),
) -> Result<ChatOutcome, AiError> {
    match settings.provider {
        ProviderKind::Anthropic => anthropic(settings, req, cancel, on_event),
        ProviderKind::OpenaiCompatible => openai(settings, req, cancel, on_event),
    }
}

fn anthropic(
    settings: &AiSettings,
    req: &ChatRequest,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(StreamEvent),
) -> Result<ChatOutcome, AiError> {
    let key = get_key(ProviderKind::Anthropic).ok_or(AiError::MissingKey("Anthropic"))?;
    let model = settings.anthropic_model.trim();
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| json!({ "role": m.role, "content": m.content }))
        .collect();
    let mut body = json!({
        "model": model,
        "max_tokens": 64000,
        "stream": true,
        "system": req.system,
        "messages": messages,
        "output_config": { "effort": settings.effort },
    });
    let base = std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| ANTHROPIC_BASE.to_owned());
    let mut request = agent()
        .post(&format!("{}/v1/messages", base.trim_end_matches('/')))
        .set("content-type", "application/json")
        .set("x-api-key", &key)
        .set("anthropic-version", "2023-06-01");
    if supports_fallback(model) {
        body["fallbacks"] = json!("default");
        request = request.set("anthropic-beta", FALLBACK_BETA);
    }
    let resp = request.send_string(&body.to_string()).map_err(http_error)?;

    let mut out = ChatOutcome {
        text: String::new(),
        model: model.to_owned(),
        stop_reason: None,
        input_tokens: 0,
        output_tokens: 0,
    };
    let mut refusal: Option<String> = None;
    for line in BufReader::new(resp.into_reader()).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AiError::Cancelled);
        }
        let line = line.map_err(|e| AiError::Network(e.to_string()))?;
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let Ok(ev) = serde_json::from_str::<Value>(data.trim()) else {
            continue;
        };
        match ev["type"].as_str().unwrap_or("") {
            "message_start" => {
                if let Some(m) = ev.pointer("/message/model").and_then(Value::as_str) {
                    if m != out.model {
                        on_event(StreamEvent::Notice(tr!("由 {m} 作答", "Answered by {m}")));
                    }
                    out.model = m.to_owned();
                }
                out.input_tokens = ev
                    .pointer("/message/usage/input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
            }
            "content_block_start" => {
                // 服务端回退的边界标记：前一个模型拒绝，后一个模型接着写
                if ev.pointer("/content_block/type").and_then(Value::as_str) == Some("fallback") {
                    let to = ev
                        .pointer("/content_block/to/model")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| tr!("其他模型", "another model"));
                    on_event(StreamEvent::Notice(tr!(
                        "原模型拒绝了这个请求，已由 {to} 继续",
                        "The original model declined this request; {to} took over"
                    )));
                }
            }
            "content_block_delta" => {
                if ev.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta") {
                    if let Some(t) = ev.pointer("/delta/text").and_then(Value::as_str) {
                        out.text.push_str(t);
                        on_event(StreamEvent::Text(t.to_owned()));
                    }
                }
            }
            "message_delta" => {
                if let Some(r) = ev.pointer("/delta/stop_reason").and_then(Value::as_str) {
                    out.stop_reason = Some(r.to_owned());
                    if r == "refusal" {
                        let cat = ev
                            .pointer("/delta/stop_details/category")
                            .and_then(Value::as_str)
                            .map(|c| tr!("（类别：{c}）", " (category: {c})"))
                            .unwrap_or_default();
                        refusal = Some(cat);
                    }
                }
                if let Some(n) = ev.pointer("/usage/output_tokens").and_then(Value::as_u64) {
                    out.output_tokens = n;
                }
            }
            "error" => {
                let message = ev
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| tr!("未知错误", "Unknown error"));
                return Err(AiError::Other(message));
            }
            _ => {}
        }
    }
    // 拒绝时已输出的部分不可作为完整结果使用
    if let Some(cat) = refusal {
        return Err(AiError::Refusal(cat));
    }
    Ok(out)
}

fn openai(
    settings: &AiSettings,
    req: &ChatRequest,
    cancel: &AtomicBool,
    on_event: &mut dyn FnMut(StreamEvent),
) -> Result<ChatOutcome, AiError> {
    let base = settings.openai_base_url.trim().trim_end_matches('/');
    let model = settings.openai_model.trim();
    if base.is_empty() || model.is_empty() {
        return Err(AiError::Other(tr!(
            "请先在 AI 设置中填写接口地址和模型名",
            "Set the API URL and model name in AI settings first"
        )));
    }
    let mut messages = vec![json!({ "role": "system", "content": req.system })];
    messages.extend(req.messages.iter().map(|m| {
        json!({ "role": match m.role { Role::User => "user", Role::Assistant => "assistant" }, "content": m.content })
    }));
    let body = json!({ "model": model, "stream": true, "messages": messages });
    let mut request = agent()
        .post(&format!("{base}/chat/completions"))
        .set("content-type", "application/json");
    // 本地 Ollama 等不需要 Key
    if let Some(key) = get_key(ProviderKind::OpenaiCompatible) {
        request = request.set("authorization", &format!("Bearer {key}"));
    }
    let resp = request.send_string(&body.to_string()).map_err(http_error)?;

    let mut out = ChatOutcome {
        text: String::new(),
        model: model.to_owned(),
        stop_reason: None,
        input_tokens: 0,
        output_tokens: 0,
    };
    for line in BufReader::new(resp.into_reader()).lines() {
        if cancel.load(Ordering::Relaxed) {
            return Err(AiError::Cancelled);
        }
        let line = line.map_err(|e| AiError::Network(e.to_string()))?;
        let Some(data) = line.strip_prefix("data:").map(str::trim) else {
            continue;
        };
        if data == "[DONE]" {
            break;
        }
        let Ok(ev) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(msg) = ev.pointer("/error/message").and_then(Value::as_str) {
            return Err(AiError::Other(msg.to_owned()));
        }
        if let Some(t) = ev
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            if !t.is_empty() {
                out.text.push_str(t);
                on_event(StreamEvent::Text(t.to_owned()));
            }
        }
        if let Some(r) = ev
            .pointer("/choices/0/finish_reason")
            .and_then(Value::as_str)
        {
            out.stop_reason = Some(r.to_owned());
        }
        if let Some(u) = ev.get("usage").filter(|u| u.is_object()) {
            out.input_tokens = u["prompt_tokens"].as_u64().unwrap_or(0);
            out.output_tokens = u["completion_tokens"].as_u64().unwrap_or(0);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_only_on_supported_models() {
        assert!(supports_fallback("claude-opus-5-5"));
        assert!(supports_fallback("claude-sonnet-5-5"));
        assert!(supports_fallback("claude-fable-5-1"));
        assert!(!supports_fallback("claude-haiku-5-5"));
        assert!(!supports_fallback("deepseek-chat"));
    }
}
