//! 编译驱动。

pub mod log;
pub mod synctex;

use serde::Serialize;
use std::path::PathBuf;
use ym_model::{Engine, Project};

#[derive(Debug, Clone, Serialize)]
pub struct CompileOutput {
    pub success: bool,
    pub pdf: Option<PathBuf>,
    pub diagnostics: Vec<log::Diagnostic>,
    pub duration_ms: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("no main file configured")]
    NoMainFile,
    #[error("engine not available: {0:?}")]
    EngineUnavailable(Engine),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// 编译项目。Tectonic 走内嵌库，其他引擎走系统 `latexmk`。
pub async fn compile(_project: &Project) -> Result<CompileOutput, CompileError> {
    // TODO(M1): Tectonic 后端（feature = "tectonic"）与 latexmk 后端；支持取消与防抖
    Err(CompileError::NoMainFile)
}
