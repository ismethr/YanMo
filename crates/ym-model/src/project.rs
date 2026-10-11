use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    /// 主文件，相对项目根目录；未设置时自动识别
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<PathBuf>,
    /// 编译引擎；未设置时自动选择
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<Engine>,
    /// 允许 shell-escape（minted、svg 等宏包需要；会让 TeX 执行外部命令）
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shell_escape: bool,
    /// 章节状态，key 为章节稳定 ID（见 `Section::id`）
    #[serde(default)]
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub section_status: BTreeMap<String, SectionStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_words: Option<u32>,
}

impl ProjectMeta {
    /// 读取 `<root>/.yanmo/project.toml`；不存在或无法解析时返回 None。
    pub fn load(root: &Path) -> Option<Self> {
        let text =
            std::fs::read_to_string(root.join(Project::META_DIR).join(Project::META_FILE)).ok()?;
        toml::from_str(&text).ok()
    }

    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let dir = root.join(Project::META_DIR);
        std::fs::create_dir_all(&dir)?;
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(
            dir.join(Project::META_FILE),
            format!("# YanMo 项目设置（可提交到版本库）\n{text}"),
        )
    }
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
