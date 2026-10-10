//! 把 TeX 日志解析为结构化诊断信息。

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    BadBox,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub message: String,
    /// 原始日志片段，供 AI 解释错误时使用
    pub raw: String,
}

pub fn parse_log(_log: &str) -> Vec<Diagnostic> {
    // TODO(M1): 跟踪文件栈括号、`! ` 错误、`l.<n>` 行号、Overfull/Underfull、undefined references
    Vec::new()
}
