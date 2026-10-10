//! AI 层。
//!
//! 铁律：AI 永远只产出 `Patch`，绝不直接写文件；由 UI 展示 diff，经用户确认后
//! 通过命令注册表应用，并进入撤销栈。

pub mod context;
pub mod provider;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 对单个文件的一处替换。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edit {
    pub file: PathBuf,
    pub start: usize,
    pub end: usize,
    pub replacement: String,
}

/// 一次 AI 建议，可包含多处修改，用户可逐块接受或拒绝。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub summary: String,
    pub edits: Vec<Edit>,
}

/// 内置的选中文本操作（AI-2），提示词模板放在 `prompts/` 下，用户可覆盖。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickAction {
    Polish,
    Academic,
    TranslateZhEn,
    TranslateEnZh,
    Shorten,
    Expand,
    ExplainError,
}
