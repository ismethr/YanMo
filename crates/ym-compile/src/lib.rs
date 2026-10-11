//! 编译驱动。
//!
//! - **Tectonic**：作为外部程序调用（官方预编译二进制，打包时作为 sidecar 随应用分发），
//!   无需安装 TeX 发行版，宏包按需下载并缓存。不以 Rust 库形式嵌入，是因为该库依赖
//!   ICU / HarfBuzz / FreeType 等系统库，会让三平台构建复杂得多。
//! - **latexmk**：使用用户已安装的 TeX Live / MacTeX / MiKTeX。
//!
//! 输出统一放在项目的 `.yanmo/build/`，不污染源码目录。

pub mod export;
pub mod log;
pub mod synctex;

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use ym_model::tr;
use ym_model::Engine;

pub use export::{ExportOutput, ExportRequest, EXPORT_DIR};
pub use log::Severity;
pub use synctex::{PdfRect, SourceLocation, SyncTex};

/// 编译产物目录，相对项目根目录。
pub const BUILD_DIR: &str = ".yanmo/build";

#[derive(Debug, Clone, Default, Serialize)]
pub struct Toolchain {
    pub tectonic: Option<PathBuf>,
    pub latexmk: Option<PathBuf>,
    /// 导出 Word
    pub pandoc: Option<PathBuf>,
    /// 导出 Word 时把 PDF/EPS 图片转为 PNG（MacTeX 与 TeX Live 通常自带）
    pub ghostscript: Option<PathBuf>,
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
    if let Some(h) = home() {
        dirs.extend([h.join(".local/bin"), h.join(".cargo/bin")]);
    }
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
            pandoc: which("pandoc"),
            ghostscript: which(if cfg!(windows) { "gswin64c" } else { "gs" }),
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
    /// 允许 TeX 执行外部命令（minted 等宏包需要）
    pub shell_escape: bool,
    /// 草稿模式：不载入图片（graphicx 的 draft 选项，图片位置画成同尺寸的框），编译更快、PDF 更小
    pub draft: bool,
    /// 超过这个时间仍未结束就停止编译
    pub timeout: Option<Duration>,
    /// 用于从其他线程停止编译
    pub cancel: CancelHandle,
}

/// 停止正在进行的编译。只结束本次编译启动的进程（及其子进程，如 xelatex、biber）。
#[derive(Debug, Clone, Default)]
pub struct CancelHandle {
    cancelled: Arc<AtomicBool>,
    pid: Arc<Mutex<Option<u32>>>,
}

impl CancelHandle {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.pid.lock().map(|p| p.is_some()).unwrap_or(false)
    }
}

/// 结束编译进程及其启动的子进程。Unix 上编译进程自成一个进程组，按组结束；Windows 用 taskkill /T。
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{}", child.id())])
            .status();
        std::thread::sleep(Duration::from_millis(300));
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .status();
    }
    let _ = child.kill();
}

/// 运行编译命令，期间响应停止请求与超时。返回输出以及是否被停止 / 超时。
fn run(mut cmd: Command, req: &CompileRequest) -> std::io::Result<(Output, Stop)> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Ok(mut p) = req.cancel.pid.lock() {
        *p = Some(child.id());
    }
    // 在后台线程读取输出，避免管道写满导致子进程阻塞
    let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let out = read(child.stdout.take().map(|p| Box::new(p) as _));
    let err = read(child.stderr.take().map(|p| Box::new(p) as _));
    let started = Instant::now();
    let mut stop = Stop::None;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if req.cancel.cancelled.load(Ordering::SeqCst) {
            stop = Stop::Cancelled;
        } else if req.timeout.is_some_and(|t| started.elapsed() > t) {
            stop = Stop::TimedOut;
        }
        if stop != Stop::None {
            kill_tree(&mut child);
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if let Ok(mut p) = req.cancel.pid.lock() {
        *p = None;
    }
    Ok((
        Output {
            status,
            stdout: out.join().unwrap_or_default(),
            stderr: err.join().unwrap_or_default(),
        },
        stop,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    None,
    Cancelled,
    TimedOut,
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
    /// 被用户停止或超时
    pub stopped: bool,
    /// 完整日志，相对项目根目录
    pub log: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("{}", tr!(
        "没有可用的编译引擎：请运行 scripts/fetch-tectonic.sh 安装 Tectonic，或安装 TeX Live / MacTeX",
        "No TeX engine available: run scripts/fetch-tectonic.sh to install Tectonic, or install TeX Live / MacTeX"
    ))]
    NoEngine,
    #[error("{}", tr!("所选引擎不可用：{e:?}", "The selected engine is not available: {e:?}", e = .0))]
    EngineUnavailable(Engine),
    #[error("{}", tr!(
        "没有找到 Pandoc：请从 https://pandoc.org/installing.html 安装后重试",
        "Pandoc not found: install it from https://pandoc.org/installing.html and try again"
    ))]
    NoPandoc,
    #[error("{}", tr!("Pandoc 转换失败：{e}", "Pandoc conversion failed: {e}", e = .0))]
    Pandoc(String),
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

/// 草稿模式在文档之前传入的选项
const DRAFT_OPTIONS: &str =
    r"\PassOptionsToPackage{draft}{graphicx}\PassOptionsToPackage{draft}{graphics}";

/// 删除编译产物目录（`.yanmo/build/`）。
pub fn clean(root: &Path) -> std::io::Result<()> {
    let dir = root.join(BUILD_DIR);
    if dir.is_dir() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

/// 执行一次编译（阻塞），期间可通过 `req.cancel` 停止。
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

    // 记录上次编译用的引擎
    let engine_mark = out_dir.join(".engine");
    let engine_name = format!("{engine:?}{}", if req.draft { "+draft" } else { "" });
    let engine_changed = std::fs::read_to_string(&engine_mark)
        .map(|e| e.trim() != engine_name)
        .unwrap_or(true);
    let _ = std::fs::write(&engine_mark, &engine_name);

    let started = Instant::now();
    let before = SystemTime::now();
    // Tectonic 没有在源码前插入代码的参数：草稿模式用一个包装文件先传入 draft 选项，再 \input 主文件。
    // 包装文件与主文件同名，生成的 PDF 名称不变；主文件用相对包装文件的路径引用，避免找回包装文件自身。
    let tectonic_input = if req.draft && engine == Engine::Tectonic {
        let dir = out_dir.join("draft");
        std::fs::create_dir_all(&dir)?;
        let depth = Path::new(BUILD_DIR).components().count() + 1;
        let main_rel = "../".repeat(depth) + &req.main.to_string_lossy().replace('\\', "/");
        let wrapper = dir.join(&file);
        std::fs::write(
            &wrapper,
            format!("{DRAFT_OPTIONS}\n\\input{{\"{main_rel}\"}}\n"),
        )?;
        wrapper.into_os_string()
    } else {
        file.clone()
    };
    let cmd = match engine {
        Engine::Tectonic => {
            let mut c = Command::new(tc.tectonic.as_ref().expect("checked"));
            c.args(["-X", "compile"])
                .arg(&tectonic_input)
                .arg("--outdir")
                .arg(&out_dir)
                // 保留 .aux：导出 Word 时用它把交叉引用替换为真实编号
                .args([
                    "--synctex",
                    "--keep-logs",
                    "--keep-intermediates",
                    "-Z",
                    "continue-on-errors",
                ])
                .current_dir(&cwd);
            if req.shell_escape {
                c.args(["-Z", "shell-escape"]);
            }
            if req.draft {
                // 包装文件不在主文件目录，让 Tectonic 也在主文件目录中查找文件
                c.arg("-Z").arg(format!("search-path={}", cwd.display()));
            }
            c
        }
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
            let mut c = Command::new(latexmk);
            c.args([
                flag,
                "-interaction=nonstopmode",
                "-file-line-error",
                "-synctex=1",
                "-f",
            ])
            .arg(format!("-outdir={}", out_dir.display()))
            .current_dir(&cwd)
            .env("PATH", std::env::join_paths(path).unwrap_or_default());
            if req.shell_escape {
                c.arg("-shell-escape");
            }
            if req.draft {
                c.arg(format!("-usepretex={DRAFT_OPTIONS}"));
            }
            // 换了引擎后强制重新编译，否则 latexmk 会把上一个引擎生成的 PDF 当作最新
            if engine_changed {
                c.arg("-g");
            }
            c.arg(&file);
            c
        }
    };
    let (output, stop) = run(cmd, req)?;
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

    match stop {
        Stop::Cancelled => diagnostics.insert(
            0,
            Diagnostic {
                severity: Severity::Error,
                file: None,
                line: None,
                message: tr!("编译已停止", "Compilation was stopped"),
            },
        ),
        Stop::TimedOut => diagnostics.insert(
            0,
            Diagnostic {
                severity: Severity::Error,
                file: None,
                line: None,
                message: tr!(
                    "编译超过 {s} 秒仍未结束，已停止（可在设置中调整超时时间）",
                    "Compilation did not finish within {s} seconds and was stopped (the timeout can be changed in Settings)",
                    s = req.timeout.map(|t| t.as_secs()).unwrap_or(0)
                ),
            },
        ),
        Stop::None => {}
    }
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
                tr!("编译失败，详见日志", "Compilation failed, see the log")
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
    // latexmk 判断无需重新编译时不会改写 PDF，此时已有的 PDF 就是最新的
    let pdf_fresh = fresh(&pdf)
        || (engine != Engine::Tectonic
            && stop == Stop::None
            && output.status.success()
            && pdf.is_file());
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
        success: stop == Stop::None
            && pdf_fresh
            && !diagnostics.iter().any(|d| d.severity == Severity::Error),
        stopped: stop != Stop::None,
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
        log: if fresh(&log_path) {
            rel(&log_path)
        } else {
            None
        },
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
            ..Default::default()
        };
        let only_tex = Toolchain {
            tectonic: None,
            latexmk: Some("l".into()),
            ..Default::default()
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
