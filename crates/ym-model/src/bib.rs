use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::span::SourceSpan;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BibEntry {
    pub key: String,
    /// article / inproceedings / book ...
    pub entry_type: String,
    /// 原样保留的字段（title, author, year, doi ...）
    pub fields: BTreeMap<String, String>,
    pub span: SourceSpan,
    /// 正文中 `\cite` 此条目的位置
    pub citations: Vec<SourceSpan>,
}

impl BibEntry {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields.get(name).map(String::as_str)
    }
}
