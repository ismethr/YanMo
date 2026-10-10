use std::path::{Component, Path, PathBuf};
use ym_compile::{CompileOutput, CompileRequest, ExportOutput, ExportRequest, SyncTex, Toolchain};
use ym_latex::ProjectIndex;
use ym_model::{Engine, ProjectMeta};

use crate::CommandError;

/// 当前打开的项目及其缓存的索引。每个宿主（桌面窗口、CLI 进程、MCP 会话）各持有一个。
#[derive(Default)]
pub struct Workspace {
    root: Option<PathBuf>,
    main: Option<PathBuf>,
    meta: ProjectMeta,
    index: Option<ProjectIndex>,
    toolchain: Option<Toolchain>,
    last_compile: Option<CompileOutput>,
    synctex: Option<SyncTex>,
}

/// 一次编译所需的全部信息。与 Workspace 分离，使宿主可以在不持有工作区锁的情况下编译。
pub struct CompileJob {
    pub request: CompileRequest,
    pub toolchain: Toolchain,
}

/// 一次 Word 导出。与 CompileJob 一样可在不持有工作区锁的情况下执行。
pub struct ExportJob {
    pub request: ExportRequest,
    pub toolchain: Toolchain,
}

impl ExportJob {
    pub fn run(&self) -> Result<ExportOutput, CommandError> {
        ym_compile::export::export_docx(&self.request, &self.toolchain)
            .map_err(|e| CommandError::Failed(e.to_string()))
    }
}

impl CompileJob {
    pub fn run(&self) -> Result<CompileOutput, CommandError> {
        ym_compile::compile(&self.request, &self.toolchain)
            .map_err(|e| CommandError::Failed(e.to_string()))
    }
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
        self.meta = ProjectMeta::load(&root).unwrap_or_default();
        // 项目设置里指定的主文件优先，失效时自动识别
        self.main = self
            .meta
            .main
            .clone()
            .filter(|m| root.join(m).is_file())
            .or_else(|| ym_latex::find_main(&root));
        self.root = Some(root);
        self.index = None;
        self.last_compile = None;
        self.synctex = None;
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

    pub fn toolchain(&mut self) -> &Toolchain {
        self.toolchain.get_or_insert_with(Toolchain::detect)
    }

    pub fn compile_job(&mut self, engine: Option<Engine>) -> Result<CompileJob, CommandError> {
        let root = self.root()?.to_path_buf();
        let main = self.main.clone().ok_or_else(|| {
            CommandError::Failed("项目中没有找到含 \\documentclass 的主文件".into())
        })?;
        Ok(CompileJob {
            request: CompileRequest { root, main, engine },
            toolchain: self.toolchain().clone(),
        })
    }

    pub fn finish_compile(&mut self, output: CompileOutput) {
        self.synctex = None;
        self.last_compile = Some(output);
    }

    pub fn last_compile(&self) -> Option<&CompileOutput> {
        self.last_compile.as_ref()
    }

    /// 最近一次编译的 PDF（相对项目根目录）；本次会话尚未编译时沿用 build 目录里已有的。
    pub fn pdf_path(&self) -> Option<String> {
        if let Some(out) = &self.last_compile {
            if out.pdf.is_some() {
                return out.pdf.clone();
            }
        }
        let stem = self
            .main
            .as_ref()?
            .file_stem()?
            .to_string_lossy()
            .into_owned();
        let rel = format!("{}/{stem}.pdf", ym_compile::BUILD_DIR);
        self.root.as_ref()?.join(&rel).is_file().then_some(rel)
    }

    pub fn synctex(&mut self) -> Result<&SyncTex, CommandError> {
        if self.synctex.is_none() {
            let root = self.root()?.to_path_buf();
            let pdf = self
                .pdf_path()
                .ok_or_else(|| CommandError::Failed("尚未编译".into()))?;
            let path = root.join(pdf.trim_end_matches(".pdf").to_owned() + ".synctex.gz");
            self.synctex = Some(SyncTex::load(&path, &root)?);
        }
        Ok(self.synctex.as_ref().expect("just set"))
    }

    /// 切换主文件（相对项目根目录）。`persist` 为真时写入 `.yanmo/project.toml`。
    pub fn set_main(&mut self, rel: &Path, persist: bool) -> Result<(), CommandError> {
        let abs = self.resolve(&rel.to_string_lossy())?;
        let is_root_doc = std::fs::read_to_string(&abs).is_ok_and(|s| {
            s.lines()
                .any(|l| l.trim_start().starts_with("\\documentclass"))
        });
        if !is_root_doc {
            return Err(CommandError::InvalidParams(format!(
                "{} 不是独立文档（没有 \\documentclass）",
                rel.display()
            )));
        }
        self.main = Some(rel.to_path_buf());
        self.index = None;
        self.last_compile = None;
        self.synctex = None;
        if persist {
            self.meta.main = self.main.clone();
            self.meta.save(self.root()?)?;
        }
        Ok(())
    }

    pub fn export_job(&mut self, output: Option<PathBuf>) -> Result<ExportJob, CommandError> {
        let root = self.root()?.to_path_buf();
        let main = self.main.clone().ok_or_else(|| {
            CommandError::Failed("项目中没有找到含 \\documentclass 的主文件".into())
        })?;
        if let Some(o) = &output {
            self.resolve(&o.to_string_lossy())?;
        }
        Ok(ExportJob {
            request: ExportRequest { root, main, output },
            toolchain: self.toolchain().clone(),
        })
    }
}
