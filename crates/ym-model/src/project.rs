use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::outline::SectionStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// 内置 Tectonic（XeTeX 内核），零配置
    #[default]
    Tectonic,
    Pdflatex,
    Xelatex,
    Lualatex,
}

/// `.yanmo/project.toml` 的内容。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// 主文件，相对项目根目录
    pub main: Option<PathBuf>,
    #[serde(default)]
    pub engine: Engine,
    /// 章节状态，key 为章节稳定 ID（见 `Section::id`）
    #[serde(default)]
    pub section_status: BTreeMap<String, SectionStatus>,
    pub target_words: Option<u32>,
}

/// 一个打开的论文项目。
#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub meta: ProjectMeta,
}

impl Project {
    pub const META_DIR: &'static str = ".yanmo";
    pub const META_FILE: &'static str = "project.toml";

    pub fn meta_path(&self) -> PathBuf {
        self.root.join(Self::META_DIR).join(Self::META_FILE)
    }
}
