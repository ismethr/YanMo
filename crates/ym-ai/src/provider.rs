//! 多厂商抽象。API Key 存放在系统钥匙串（由宿主注入），不写入配置文件。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProviderConfig {
    Anthropic { model: String },
    /// 覆盖 OpenAI / DeepSeek / 通义 / Ollama / vLLM 等任意兼容接口
    OpenAiCompatible { base_url: String, model: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("missing api key")]
    MissingKey,
    #[error("http {status}: {body}")]
    Http { status: u16, body: String },
}

/// 流式输出通过回调逐段返回，便于 UI 实时渲染。
pub trait Provider: Send + Sync {
    fn chat(
        &self,
        messages: &[Message],
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<String, ProviderError>;
}
