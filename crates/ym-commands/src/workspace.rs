use std::path::{Component, Path, PathBuf};
use ym_compile::{CompileOutput, CompileRequest, ExportOutput, ExportRequest, SyncTex, Toolchain};
use ym_latex::ProjectIndex;
use ym_model::{tr, Engine, ProjectMeta};

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
    /// 当前（或最近一次）编译的停止句柄
    compile_cancel: ym_compile::CancelHandle,
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
            return Err(CommandError::Failed(tr!(
                "不是文件夹：{p}",
                "Not a folder: {p}",
                p = root.display()
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
            let main = self
                .main
                .clone()
                .ok_or_else(|| CommandError::Failed(no_main()))?;
            self.index = Some(ym_latex::index_project(&root, &main)?);
        }
        Ok(self.index.as_ref().expect("just set"))
    }

    /// 项目引用的全部文献条目；`keys` 非空时只取这些。
    pub fn bib_entries(
        &mut self,
        keys: &[String],
    ) -> Result<Vec<ym_model::BibEntry>, CommandError> {
        let index = self.index()?;
        let report = ym_check::run(index);
        Ok(report
            .bib
            .into_iter()
            .filter(|e| keys.is_empty() || keys.contains(&e.key))
            .collect())
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
            return Err(CommandError::InvalidParams(tr!(
                "路径必须位于项目内：{rel}",
                "The path must be inside the project: {rel}"
            )));
        }
        Ok(self.root()?.join(p))
    }

    pub fn toolchain(&mut self) -> &Toolchain {
        self.toolchain.get_or_insert_with(Toolchain::detect)
    }

    /// 停止正在进行的编译；没有在编译时返回 false。
    pub fn cancel_compile(&self) -> bool {
        let running = self.compile_cancel.is_running();
        self.compile_cancel.cancel();
        running
    }

    /// 项目设置（`.yanmo/project.toml`）
    pub fn meta(&self) -> &ProjectMeta {
        &self.meta
    }

    /// 修改并保存项目设置
    pub fn update_meta(&mut self, f: impl FnOnce(&mut ProjectMeta)) -> Result<(), CommandError> {
        f(&mut self.meta);
        let root = self.root()?.to_path_buf();
        self.meta.save(&root)?;
        Ok(())
    }

    /// 重新检测编译工具（安装了新的 TeX 发行版之后）
    pub fn redetect_toolchain(&mut self) -> &Toolchain {
        self.toolchain = None;
        self.toolchain()
    }

    /// 准备一次编译。未指定引擎时使用项目设置，再没有则自动选择。
    pub fn compile_job(
        &mut self,
        engine: Option<Engine>,
        timeout: Option<std::time::Duration>,
    ) -> Result<CompileJob, CommandError> {
        let root = self.root()?.to_path_buf();
        let main = self
            .main
            .clone()
            .ok_or_else(|| CommandError::Failed(no_main()))?;
        Ok(CompileJob {
            request: CompileRequest {
                root,
                main,
                engine: engine.or(self.meta.engine),
                shell_escape: self.meta.shell_escape,
                timeout,
                cancel: {
                    self.compile_cancel = ym_compile::CancelHandle::default();
                    self.compile_cancel.clone()
                },
            },
            toolchain: self.toolchain().clone(),
        })
    }

    pub fn finish_compile(&mut self, output: CompileOutput) {
        self.synctex = None;
        self.last_compile = Some(output);
    }

    /// 编译缓存被清理后，忘掉上次的结果
    pub fn finish_compile_cleared(&mut self) {
        self.synctex = None;
        self.last_compile = None;
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
                .ok_or_else(|| CommandError::Failed(tr!("尚未编译", "Not compiled yet")))?;
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
            return Err(CommandError::InvalidParams(tr!(
                "{p} 不是独立文档（没有 \\documentclass）",
                "{p} is not a standalone document (no \\documentclass)",
                p = rel.display()
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
        let main = self
            .main
            .clone()
            .ok_or_else(|| CommandError::Failed(no_main()))?;
        if let Some(o) = &output {
            self.resolve(&o.to_string_lossy())?;
        }
        Ok(ExportJob {
            request: ExportRequest { root, main, output },
            toolchain: self.toolchain().clone(),
        })
    }
}

fn no_main() -> String {
    tr!(
        "项目中没有找到含 \\documentclass 的主文件",
        "No main file with \\documentclass found in the project"
    )
}
