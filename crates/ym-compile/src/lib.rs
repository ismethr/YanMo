//! 编译驱动。
//!
//! - **Tectonic**：作为外部程序调用（官方预编译二进制，打包时作为 sidecar 随应用分发），
//!   无需安装 TeX 发行版，宏包按需下载并缓存。不以 Rust 库形式嵌入，是因为该库依赖
//!   ICU / HarfBuzz / FreeType 等系统库，会让三平台构建复杂得多。
//! - **latexmk**：使用用户已安装的 TeX Live / MacTeX / MiKTeX。
//!
//! 输出统一放在项目的 `.yanmo/build/`，不污染源码目录。

pub mod log;
pub mod synctex;

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime};
use ym_model::Engine;

pub use log::Severity;
pub use synctex::{PdfRect, SourceLocation, SyncTex};

/// 编译产物目录，相对项目根目录。
pub const BUILD_DIR: &str = ".yanmo/build";

#[derive(Debug, Clone, Default, Serialize)]
pub struct Toolchain {
    pub tectonic: Option<PathBuf>,
    pub latexmk: Option<PathBuf>,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

/// 在 PATH 与常见 TeX 安装目录中查找可执行文件。
/// 从 Finder 启动的 macOS 应用拿不到 shell 的 PATH，所以要补上 TeX 的常见位置。
fn which(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.extend(
        [
            "/Library/TeX/texbin",
            "/usr/texbin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
        ]
        .iter()
        .map(PathBuf::from),
    );
    dirs.into_iter()
        .map(|d| d.join(exe(name)))
        .find(|p| p.is_file())
}

impl Toolchain {
    /// 查找顺序：环境变量 `YANMO_TECTONIC` → 与本程序同目录（打包的 sidecar）→ `~/.yanmo/bin` → PATH。
    pub fn detect() -> Self {
        let tectonic = std::env::var_os("YANMO_TECTONIC")
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| {
                let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
                Some(dir.join(exe("tectonic"))).filter(|p| p.is_file())
            })
            .or_else(|| {
                Some(home()?.join(".yanmo/bin").join(exe("tectonic"))).filter(|p| p.is_file())
            })
            .or_else(|| which("tectonic"));
        Self {
            tectonic,
            latexmk: which("latexmk"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompileRequest {
    pub root: PathBuf,
    /// 相对 `root`
    pub main: PathBuf,
    /// 不指定时自动选择，见 [`choose_engine`]
    pub engine: Option<Engine>,
}

/// 一条诊断；`file` 已解析为相对项目根目录的路径（项目外的文件为 None）。
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompileOutput {
    pub success: bool,
    pub engine: Engine,
    /// 本次生成的 PDF，相对项目根目录
    pub pdf: Option<String>,
    pub synctex: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub duration_ms: u64,
    /// 日志末尾，便于界面展示和 AI 解释错误
    pub log_tail: String,
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("没有可用的编译引擎：请运行 scripts/fetch-tectonic.sh 安装 Tectonic，或安装 TeX Live / MacTeX")]
    NoEngine,
    #[error("所选引擎不可用：{0:?}")]
    EngineUnavailable(Engine),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// 读取主文件开头的 `% !TEX program = xxx` 魔法注释。
fn magic_program(source: &str) -> Option<Engine> {
    source.lines().take(20).find_map(|l| {
        let l = l.trim_start().strip_prefix('%')?.trim_start();
        let l = l.strip_prefix("!TEX")?.trim_start();
        let (key, value) = l.split_once('=')?;
        if !key.trim().eq_ignore_ascii_case("program")
            && !key.trim().eq_ignore_ascii_case("TS-program")
        {
            return None;
        }
        match value.trim().to_ascii_lowercase().as_str() {
            "pdflatex" => Some(Engine::Pdflatex),
            "xelatex" => Some(Engine::Xelatex),
            "lualatex" => Some(Engine::Lualatex),
            "tectonic" => Some(Engine::Tectonic),
            _ => None,
        }
    })
}

/// 选择引擎：明确指定 > 魔法注释要求的 pdfLaTeX/LuaLaTeX（需 latexmk）> Tectonic > latexmk + XeLaTeX。
/// Tectonic 基于 XeTeX，所以 `program = xelatex` 时也优先用 Tectonic。
pub fn choose_engine(
    requested: Option<Engine>,
    source: &str,
    tc: &Toolchain,
) -> Result<Engine, CompileError> {
    let available = |e: Engine| match e {
        Engine::Tectonic => tc.tectonic.is_some(),
        _ => tc.latexmk.is_some(),
    };
    if let Some(e) = requested {
        return if available(e) {
            Ok(e)
        } else {
            Err(CompileError::EngineUnavailable(e))
        };
    }
    if let Some(e @ (Engine::Pdflatex | Engine::Lualatex)) = magic_program(source) {
        if available(e) {
            return Ok(e);
        }
    }
    if available(Engine::Tectonic) {
        Ok(Engine::Tectonic)
    } else if available(Engine::Xelatex) {
        Ok(Engine::Xelatex)
    } else {
        Err(CompileError::NoEngine)
    }
}

fn modified(p: &Path) -> Option<SystemTime> {
    p.metadata().ok()?.modified().ok()
}

fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

/// 把 TeX 看到的文件名（相对主文件目录，可能省略 .tex）解析为项目内的相对路径。
fn resolve_source(root: &Path, main_dir: &Path, name: &str) -> Option<String> {
    let name = name.trim().trim_start_matches("./");
    let p = Path::new(name);
    let rel = if p.is_absolute() {
        p.strip_prefix(root).ok()?.to_path_buf()
    } else {
        main_dir.join(p)
    };
    let mut candidates = vec![rel.clone()];
    if rel.extension().is_none() {
        candidates.insert(0, rel.with_extension("tex"));
    }
    candidates
        .into_iter()
        .find(|c| root.join(c).is_file())
        .map(|c| c.to_string_lossy().replace('\\', "/"))
}

/// 执行一次编译（阻塞）。
pub fn compile(req: &CompileRequest, tc: &Toolchain) -> Result<CompileOutput, CompileError> {
    let main_abs = req.root.join(&req.main);
    let source = std::fs::read_to_string(&main_abs)?;
    let engine = choose_engine(req.engine, &source, tc)?;

    let main_dir_rel = req.main.parent().map(Path::to_path_buf).unwrap_or_default();
    let cwd = req.root.join(&main_dir_rel);
    let file = req.main.file_name().expect("main is a file").to_owned();
    let stem = req
        .main
        .file_stem()
        .expect("main is a file")
        .to_string_lossy()
        .into_owned();
    let out_dir = req.root.join(BUILD_DIR);
    std::fs::create_dir_all(&out_dir)?;
    let pdf = out_dir.join(format!("{stem}.pdf"));
    let log_path = out_dir.join(format!("{stem}.log"));
    let synctex_path = out_dir.join(format!("{stem}.synctex.gz"));

    let started = Instant::now();
    let before = SystemTime::now();
    let output = match engine {
        Engine::Tectonic => Command::new(tc.tectonic.as_ref().expect("checked"))
            .args(["-X", "compile"])
            .arg(&file)
            .arg("--outdir")
            .arg(&out_dir)
            .args(["--synctex", "--keep-logs", "-Z", "continue-on-errors"])
            .current_dir(&cwd)
            .output()?,
        _ => {
            let latexmk = tc.latexmk.as_ref().expect("checked");
            let flag = match engine {
                Engine::Pdflatex => "-pdf",
                Engine::Lualatex => "-lualatex",
                _ => "-xelatex",
            };
            // 让 latexmk 能找到同目录下的 xelatex / biber 等
            let mut path = vec![latexmk.parent().unwrap_or(Path::new("/")).to_path_buf()];
            if let Some(p) = std::env::var_os("PATH") {
                path.extend(std::env::split_paths(&p));
            }
            Command::new(latexmk)
                .args([
                    flag,
                    "-interaction=nonstopmode",
                    "-file-line-error",
                    "-synctex=1",
                    "-f",
                ])
                .arg(format!("-outdir={}", out_dir.display()))
                .arg(&file)
                .current_dir(&cwd)
                .env("PATH", std::env::join_paths(path).unwrap_or_default())
                .output()?
        }
    };
    let duration_ms = started.elapsed().as_millis() as u64;

    let fresh = |p: &Path| modified(p).is_some_and(|m| m >= before);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let log_text = if fresh(&log_path) {
        std::fs::read_to_string(&log_path).unwrap_or_default()
    } else {
        String::new()
    };

    let mut diagnostics: Vec<Diagnostic> = log::parse_log(&log_text)
        .into_iter()
        .map(|d| Diagnostic {
            severity: d.severity,
            file: d
                .file
                .as_deref()
                .and_then(|f| resolve_source(&req.root, &main_dir_rel, f)),
            line: d.line,
            message: d.message,
        })
        .collect();

    // 日志之外的致命错误（如 Tectonic 下载宏包失败）
    let has_error = diagnostics.iter().any(|d| d.severity == Severity::Error);
    if !has_error && !output.status.success() {
        let fatal: Vec<&str> = stderr
            .lines()
            .filter(|l| l.starts_with("error:") || l.starts_with("caused by:"))
            .collect();
        diagnostics.push(Diagnostic {
            severity: Severity::Error,
            file: None,
            line: None,
            message: if fatal.is_empty() {
                "编译失败，详见日志".to_owned()
            } else {
                fatal.join(" ")
            },
        });
    }

    let rel = |p: &Path| {
        p.strip_prefix(&req.root)
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
    };
    let pdf_fresh = fresh(&pdf);
    let log_tail = match engine {
        Engine::Tectonic => {
            let noise = |l: &&str| {
                !l.contains("accessing absolute path") && !l.starts_with("note: downloading")
            };
            tail(
                &stderr.lines().filter(noise).collect::<Vec<_>>().join("\n"),
                40,
            )
        }
        _ => tail(
            if log_text.is_empty() {
                &stdout
            } else {
                &log_text
            },
            40,
        ),
    };
    Ok(CompileOutput {
        success: pdf_fresh && !diagnostics.iter().any(|d| d.severity == Severity::Error),
        engine,
        pdf: if pdf_fresh { rel(&pdf) } else { None },
        synctex: if fresh(&synctex_path) {
            rel(&synctex_path)
        } else {
            None
        },
        diagnostics,
        duration_ms,
        log_tail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_choice() {
        let both = Toolchain {
            tectonic: Some("t".into()),
            latexmk: Some("l".into()),
        };
        let only_tex = Toolchain {
            tectonic: None,
            latexmk: Some("l".into()),
        };
        let none = Toolchain::default();
        assert_eq!(
            choose_engine(None, "\\documentclass{article}", &both).unwrap(),
            Engine::Tectonic
        );
        assert_eq!(
            choose_engine(None, "% !TEX program = xelatex\n", &both).unwrap(),
            Engine::Tectonic
        );
        assert_eq!(
            choose_engine(None, "% !TEX program = pdflatex\n", &both).unwrap(),
            Engine::Pdflatex
        );
        assert_eq!(choose_engine(None, "", &only_tex).unwrap(), Engine::Xelatex);
        assert!(matches!(
            choose_engine(None, "", &none),
            Err(CompileError::NoEngine)
        ));
        assert!(matches!(
            choose_engine(Some(Engine::Tectonic), "", &only_tex),
            Err(CompileError::EngineUnavailable(Engine::Tectonic))
        ));
    }
}
