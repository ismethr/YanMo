use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 源码位置，所有模型对象都能定位回源码（用于跳转与 SyncTeX）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: PathBuf,
    /// 0-based 字节偏移
    pub start: usize,
    pub end: usize,
    /// 1-based 行号，便于展示
    pub line: u32,
}
