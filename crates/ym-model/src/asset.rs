use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::span::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Figure,
    Table,
    Equation,
    Algorithm,
    Listing,
}

/// 图表等可被 `\ref` 的资产。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub kind: AssetKind,
    pub label: Option<String>,
    pub caption: Option<String>,
    /// figure 中 `\includegraphics` 引用的文件
    pub files: Vec<PathBuf>,
    pub span: SourceSpan,
    /// 正文中引用此资产的位置
    pub references: Vec<SourceSpan>,
}
