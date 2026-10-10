use std::path::{Component, Path, PathBuf};
use ym_latex::ProjectIndex;

use crate::CommandError;

/// 当前打开的项目及其缓存的索引。每个宿主（桌面窗口、CLI 进程、MCP 会话）各持有一个。
#[derive(Default)]
pub struct Workspace {
    root: Option<PathBuf>,
    main: Option<PathBuf>,
    index: Option<ProjectIndex>,
}

impl Workspace {
    pub fn open(&mut self, root: &Path) -> Result<(), CommandError> {
        if !root.is_dir() {
            return Err(CommandError::Failed(format!(
                "不是文件夹：{}",
                root.display()
            )));
        }
        let root = root.canonicalize()?;
        self.main = ym_latex::find_main(&root);
        self.root = Some(root);
        self.index = None;
        Ok(())
    }

    pub fn root(&self) -> Result<&Path, CommandError> {
        self.root.as_deref().ok_or(CommandError::NoProject)
    }

    pub fn main(&self) -> Option<&Path> {
        self.main.as_deref()
    }

    /// 返回最新索引，必要时重新解析。
    pub fn index(&mut self) -> Result<&ProjectIndex, CommandError> {
        if self.index.is_none() {
            let root = self.root()?.to_path_buf();
            let main = self.main.clone().ok_or_else(|| {
                CommandError::Failed("项目中没有找到含 \\documentclass 的主文件".into())
            })?;
            self.index = Some(ym_latex::index_project(&root, &main)?);
        }
        Ok(self.index.as_ref().expect("just set"))
    }

    /// 文件有改动，下次访问时重新索引。
    pub fn invalidate(&mut self) {
        self.index = None;
    }

    /// 把项目内相对路径解析为绝对路径；拒绝绝对路径与 `..`，防止越出项目目录。
    pub fn resolve(&self, rel: &str) -> Result<PathBuf, CommandError> {
        let p = Path::new(rel);
        let safe = p
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if rel.is_empty() || !safe {
            return Err(CommandError::InvalidParams(format!(
                "路径必须位于项目内：{rel}"
            )));
        }
        Ok(self.root()?.join(p))
    }
}
