//! AI 设置。非敏感部分存 `~/.yanmo/ai.json`；API Key 只存系统钥匙串
//! （macOS 钥匙串、Windows 凭据管理器、Linux 内核密钥环），不写入任何文件，界面也拿不到明文。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::AiError;
use ym_model::tr;

const KEYCHAIN_SERVICE: &str = "app.yanmo.desktop";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Anthropic 原生 Messages 接口
    Anthropic,
    /// OpenAI Chat Completions 兼容接口：OpenAI、DeepSeek、通义、Ollama、vLLM……
    OpenaiCompatible,
}

impl ProviderKind {
    pub fn label(self) -> &'static str {
        match (self, ym_model::lang()) {
            (ProviderKind::Anthropic, _) => "Anthropic",
            (ProviderKind::OpenaiCompatible, ym_model::Lang::Zh) => "OpenAI 兼容接口",
            (ProviderKind::OpenaiCompatible, ym_model::Lang::En) => "OpenAI-compatible API",
        }
    }

    fn keychain_user(self) -> &'static str {
        match self {
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::OpenaiCompatible => "openai-compatible",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    pub provider: ProviderKind,
    /// Anthropic 模型
    pub anthropic_model: String,
    /// 思考深度：low / medium / high / xhigh / max
    pub effort: String,
    /// OpenAI 兼容接口地址（含 /v1），例如 https://api.deepseek.com/v1、http://localhost:11434/v1
    pub openai_base_url: String,
    pub openai_model: String,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            provider: ProviderKind::Anthropic,
            anthropic_model: "claude-opus-5-5".into(),
            effort: "medium".into(),
            openai_base_url: "http://localhost:11434/v1".into(),
            openai_model: String::new(),
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".yanmo").join("ai.json"))
}

impl AiSettings {
    pub fn load() -> Self {
        settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), AiError> {
        let path = settings_path().ok_or_else(|| {
            AiError::Other(tr!("找不到用户目录", "Cannot find the home directory"))
        })?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| AiError::Other(e.to_string()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| AiError::Other(e.to_string()))?;
        std::fs::write(path, text).map_err(|e| AiError::Other(e.to_string()))
    }

    pub fn model(&self) -> &str {
        match self.provider {
            ProviderKind::Anthropic => &self.anthropic_model,
            ProviderKind::OpenaiCompatible => &self.openai_model,
        }
    }
}

fn entry(kind: ProviderKind) -> Result<keyring::Entry, AiError> {
    keyring::Entry::new(KEYCHAIN_SERVICE, kind.keychain_user())
        .map_err(|e| AiError::Keychain(e.to_string()))
}

/// 把 API Key 存入系统钥匙串；传空字符串表示删除。
pub fn set_key(kind: ProviderKind, key: &str) -> Result<(), AiError> {
    let e = entry(kind)?;
    let key = key.trim();
    if key.is_empty() {
        match e.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(AiError::Keychain(err.to_string())),
        }
    } else {
        e.set_password(key)
            .map_err(|err| AiError::Keychain(err.to_string()))
    }
}

/// 读取 API Key：先看环境变量（ANTHROPIC_API_KEY / OPENAI_API_KEY），再看钥匙串。
pub fn get_key(kind: ProviderKind) -> Option<String> {
    let env = match kind {
        ProviderKind::Anthropic => "ANTHROPIC_API_KEY",
        ProviderKind::OpenaiCompatible => "OPENAI_API_KEY",
    };
    if let Some(v) = std::env::var(env).ok().filter(|v| !v.trim().is_empty()) {
        return Some(v);
    }
    entry(kind)
        .ok()?
        .get_password()
        .ok()
        .filter(|k| !k.is_empty())
}
