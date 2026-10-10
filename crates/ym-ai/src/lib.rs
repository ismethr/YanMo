//! AI 服务。
//!
//! 铁律：AI 只产出文本或修改建议，**从不直接写文件**。修改建议由界面以 diff 呈现，
//! 用户接受后才作为一次普通编辑应用（可撤销）。
//!
//! - [`settings`]：服务商与模型设置（`~/.yanmo/ai.json`），API Key 只存系统钥匙串；
//! - [`provider`]：Anthropic 原生 Messages 接口与 OpenAI 兼容接口的流式调用；
//! - [`prompts`]：写作任务（润色、翻译、解释错误……）的提示词。

pub mod prompts;
pub mod provider;
pub mod settings;

use serde::{Deserialize, Serialize};
use ym_model::tr;

pub use provider::{stream, ChatOutcome, StreamEvent};
pub use settings::{AiSettings, ProviderKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// 一次对话请求：系统提示词 + 多轮消息（只含文本）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<Message>,
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("{}", tr!("还没有设置 {p} 的 API Key", "No API key set for {p}", p = .0))]
    MissingKey(&'static str),
    #[error("{}", tr!("服务商返回错误（HTTP {status}）：{message}", "The provider returned an error (HTTP {status}): {message}"))]
    Http { status: u16, message: String },
    #[error("{}", tr!("网络错误：{e}", "Network error: {e}", e = .0))]
    Network(String),
    #[error("{}", tr!("模型拒绝了这个请求{c}", "The model declined this request{c}", c = .0))]
    Refusal(String),
    #[error("{}", tr!("已取消", "Cancelled"))]
    Cancelled,
    #[error("{}", tr!("钥匙串错误：{e}", "Keychain error: {e}", e = .0))]
    Keychain(String),
    #[error("{0}")]
    Other(String),
}
