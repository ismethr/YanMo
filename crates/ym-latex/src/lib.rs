//! 结构级 LaTeX 解析器。
//!
//! 不做宏展开，只做容错扫描，提取写作管理需要的结构信息。
//! 必须能处理"写到一半"的非法源码，任何输入都不能 panic。
//!
//! - [`parse_file`]：解析单个文件，得到 [`FileIndex`]
//! - [`index_project`]：从主文件出发沿 `\input`/`\include` 递归，得到 [`ProjectIndex`]

mod project;
mod scanner;
pub mod text;

use serde::Serialize;
use std::path::{Path, PathBuf};
use ym_model::{Asset, SectionLevel, SourceSpan};

pub use project::{find_main, find_roots, index_project, ProjectIndex, GRAPHIC_EXTS};

/// 带源码位置的名字（label 名、bib key、文件路径等）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Located {
    pub name: String,
    pub span: SourceSpan,
}

/// 文件内的一个章节标题（扁平，未组装成树）。
#[derive(Debug, Clone, Serialize)]
pub struct Heading {
    pub level: SectionLevel,
    pub title: String,
    pub label: Option<String>,
    pub starred: bool,
    pub span: SourceSpan,
}

/// 文件内容按文档顺序展开的事件流，用于跨文件拼装大纲并归属字数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum FlowItem {
    /// 一段正文的字数
    Words(u32),
    /// `headings[i]`
    Heading(usize),
    /// 正文中的 `includes[i]`
    Include(usize),
    /// 导言区中的 `includes[i]`（只索引，不计字数、不进大纲）
    PreambleInclude(usize),
}

/// 单个文件的解析结果。
#[derive(Debug, Clone, Default, Serialize)]
pub struct FileIndex {
    pub path: PathBuf,
    /// 文件含 `\documentclass`
    pub is_root_document: bool,
    pub headings: Vec<Heading>,
    pub assets: Vec<Asset>,
    pub labels: Vec<Located>,
    /// `\ref` `\eqref` `\cref` `\autoref` 等
    pub refs: Vec<Located>,
    /// `\cite` `\citep` `\parencite` 等
    pub cites: Vec<Located>,
    /// 出现了 `\nocite{*}`
    pub nocite_all: bool,
    /// `thebibliography` 里的 `\bibitem`
    pub bibitems: Vec<Located>,
    /// `\input` / `\include` / `\subfile` / `\import` 的目标
    pub includes: Vec<Located>,
    /// `\bibliography` / `\addbibresource` 的目标
    pub bib_files: Vec<Located>,
    /// `\includegraphics` 的目标
    pub graphics: Vec<Located>,
    /// `\graphicspath` 中的目录
    pub graphics_paths: Vec<String>,
    pub flow: Vec<FlowItem>,
}

/// 解析单个 tex 文件。`path` 原样写入各 [`SourceSpan`]，建议传相对项目根目录的路径。
pub fn parse_file(path: &Path, source: &str) -> FileIndex {
    scanner::Parser::new(path, source).run()
}
