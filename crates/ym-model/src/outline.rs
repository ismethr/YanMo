use serde::{Deserialize, Serialize};

use crate::span::SourceSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionLevel {
    Part,
    Chapter,
    Section,
    Subsection,
    Subsubsection,
    Paragraph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionStatus {
    #[default]
    Todo,
    Draft,
    NeedsRevision,
    Done,
}

/// 大纲树节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// 稳定 ID：优先使用章节 `\label`，否则由标题路径哈希生成
    pub id: String,
    pub level: SectionLevel,
    pub title: String,
    pub label: Option<String>,
    pub span: SourceSpan,
    /// 本节正文字数（不含子节）
    pub words: u32,
    pub children: Vec<Section>,
}
