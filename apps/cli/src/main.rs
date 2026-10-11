use anyhow::Context;
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use ym_check::{Issue, Severity};
use ym_model::{tr, Lang, Section};

#[derive(Parser)]
#[command(
    name = "ym",
    version,
    about = "YanMo (研墨) command line: check, compile and export LaTeX papers"
)]
struct Cli {
    /// Project root directory
    #[arg(short = 'C', long, default_value = ".")]
    project: PathBuf,
    /// Main file, relative to the project root (detected automatically by default)
    #[arg(long)]
    main: Option<PathBuf>,
    /// Print JSON, for scripts and agents
    #[arg(long)]
    json: bool,
    /// Message language: zh or en (defaults to $YM_LANG, then the system locale)
    #[arg(long, global = true)]
    lang: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compile the paper into .yanmo/build/; exits with 1 on errors
    Compile {
        /// Engine: tectonic / xelatex / pdflatex / lualatex (chosen automatically by default)
        #[arg(long)]
        engine: Option<String>,
    },
    /// Print the outline and word counts
    Outline,
    /// Check references, citations and images; exits with 1 on errors (for CI)
    Check {
        /// Treat warnings as failures too
        #[arg(long)]
        strict: bool,
    },
    /// Export to Word (.docx) with Pandoc; compile first so cross-references show numbers
    Export {
        /// Output path, relative to the project root (default .yanmo/export/<main>.docx)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Import a reference by DOI or arXiv ID
    BibImport { id: String },
    /// Run an MCP server on stdio for Claude Code, Codex and other agents, e.g.
    /// `claude mcp add yanmo -- ym mcp -C /path/to/paper`
    Mcp,
    /// Run any registered command, e.g. `ym exec project.index`; lists all commands without an ID
    Exec {
        id: Option<String>,
        /// JSON parameters, e.g. '{"path":"sections/intro.tex"}'
        params: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let lang = cli
        .lang
        .clone()
        .or_else(|| std::env::var("YM_LANG").ok())
        .and_then(|l| Lang::parse(&l))
        .unwrap_or_else(Lang::from_env);
    ym_model::set_lang(lang);
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{}{e:#}", tr!("错误：", "error: "));
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let root = &cli.project;
    let open = || -> anyhow::Result<ym_latex::ProjectIndex> {
        let main = match &cli.main {
            Some(m) => m.clone(),
            None => ym_latex::find_main(root).with_context(|| {
                tr!(
                    "在 {r} 中找不到含 \\documentclass 的主文件，可用 --main 指定",
                    "No main file with \\documentclass found in {r}; use --main to choose one",
                    r = root.display()
                )
            })?,
        };
        ym_latex::index_project(root, &main).with_context(|| {
            tr!(
                "无法读取主文件 {p}",
                "Cannot read the main file {p}",
                p = root.join(&main).display()
            )
        })
    };

    match cli.cmd {
        Cmd::Outline => {
            let project = open()?;
            if cli.json {
                print_json(&OutlineJson {
                    main: &project.main,
                    total_words: project.total_words(),
                    front_words: project.front_words,
                    outline: &project.outline,
                })?;
            } else {
                println!(
                    "{}  {}",
                    project.main.display(),
                    tr!("共 {n} 字", "{n} words total", n = project.total_words())
                );
                if project.front_words > 0 {
                    println!(
                        "  {}",
                        tr!(
                            "（前置内容）  {n} 字",
                            "(front matter)  {n} words",
                            n = project.front_words
                        )
                    );
                }
                print_outline(&project.outline, 1);
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Check { strict } => {
            let project = open()?;
            let report = ym_check::run(&project);
            let errors = report.count(Severity::Error);
            let warnings = report.count(Severity::Warning);
            if cli.json {
                let issues: Vec<_> = report.issues.iter().map(IssueJson::from).collect();
                print_json(&CheckJson {
                    errors,
                    warnings,
                    issues,
                })?;
            } else {
                for issue in &report.issues {
                    println!("{}", format_issue(issue));
                }
                if report.issues.is_empty() {
                    println!("{}", tr!("✓ 未发现问题", "✓ No problems found"));
                } else {
                    println!(
                        "\n{}",
                        tr!(
                            "{errors} 个错误，{warnings} 个警告",
                            "{errors} errors, {warnings} warnings"
                        )
                    );
                }
            }
            let failed = errors > 0 || (strict && warnings > 0);
            Ok(if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
        Cmd::Mcp => {
            let mut server = ym_mcp::Server::new(Some(root), cli.main.as_deref())?;
            ym_mcp::serve(
                &mut server,
                std::io::stdin().lock(),
                std::io::stdout().lock(),
            )?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Exec { id, params } => {
            let registry = ym_commands::Registry::with_builtins();
            let Some(id) = id else {
                for m in registry.list() {
                    let mark = if m.mutating {
                        tr!("  [修改文件]", "  [writes files]")
                    } else {
                        String::new()
                    };
                    let title = match ym_model::lang() {
                        Lang::Zh => m.title,
                        Lang::En => m.title_en,
                    };
                    println!("{:<18} {title}{mark}", m.id);
                }
                return Ok(ExitCode::SUCCESS);
            };
            let mut ws = ym_commands::Workspace::default();
            ws.open(root)?;
            if let Some(m) = &cli.main {
                ws.set_main(m, false)?;
            }
            let params = match params {
                Some(p) => serde_json::from_str(&p)
                    .with_context(|| tr!("参数不是合法 JSON", "Parameters are not valid JSON"))?,
                None => serde_json::Value::Null,
            };
            print_json(&registry.execute(&mut ws, &id, params)?)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Export { output } => {
            let registry = ym_commands::Registry::with_builtins();
            let mut ws = ym_commands::Workspace::default();
            ws.open(root)?;
            if let Some(m) = &cli.main {
                ws.set_main(m, false)?;
            }
            let out = registry.execute(
                &mut ws,
                "export.docx",
                serde_json::json!({ "output": output }),
            )?;
            if cli.json {
                print_json(&out)?;
            } else {
                for w in out["warnings"].as_array().into_iter().flatten() {
                    println!("{}", w.as_str().unwrap_or(""));
                }
                for m in out["figures_missing"].as_array().into_iter().flatten() {
                    println!(
                        "{}",
                        tr!(
                            "⚠ 图片未能转换，Word 中会缺失：{f}",
                            "⚠ Image could not be converted and will be missing in Word: {f}",
                            f = m.as_str().unwrap_or("")
                        )
                    );
                }
                if out["numbered_refs"] != true {
                    println!(
                        "{}",
                        tr!(
                            "⚠ 没有找到编译生成的 .aux，交叉引用会显示为标签名；先运行 ym compile",
                            "⚠ No .aux from a previous build, so cross-references will show label names; run ym compile first"
                        )
                    );
                }
                let secs = out["duration_ms"].as_u64().unwrap_or(0) as f64 / 1000.0;
                println!(
                    "\n{}",
                    tr!(
                        "✓ 已导出 {p}（{l} 个引用编号，转换 {f} 张图片，{secs:.1}s）",
                        "✓ Exported {p} ({l} reference numbers, {f} images converted, {secs:.1}s)",
                        p = out["path"].as_str().unwrap_or(""),
                        l = out["labels"],
                        f = out["figures_converted"]
                    )
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Compile { engine } => {
            let registry = ym_commands::Registry::with_builtins();
            let mut ws = ym_commands::Workspace::default();
            ws.open(root)?;
            if let Some(m) = &cli.main {
                ws.set_main(m, false)?;
            }
            let params = serde_json::json!({ "engine": engine });
            let out = registry.execute(&mut ws, "compile.run", params)?;
            if cli.json {
                print_json(&out)?;
            } else {
                for d in out["diagnostics"].as_array().into_iter().flatten() {
                    let loc = match (d["file"].as_str(), d["line"].as_u64()) {
                        (Some(f), Some(l)) => format!("{f}:{l}: "),
                        (Some(f), None) => format!("{f}: "),
                        _ => String::new(),
                    };
                    println!(
                        "{loc}{}: {}",
                        d["severity"].as_str().unwrap_or(""),
                        d["message"].as_str().unwrap_or("")
                    );
                }
                let secs = out["duration_ms"].as_u64().unwrap_or(0) as f64 / 1000.0;
                let engine = out["engine"].as_str().unwrap_or("");
                match out["pdf"].as_str() {
                    Some(pdf) => println!(
                        "\n{}",
                        if out["success"] == true {
                            tr!(
                                "✓ 已生成 {pdf}（{engine}，{secs:.1}s）",
                                "✓ Built {pdf} ({engine}, {secs:.1}s)"
                            )
                        } else {
                            tr!(
                                "⚠ 有错误，仍生成了 {pdf}（{engine}，{secs:.1}s）",
                                "⚠ Built {pdf} with errors ({engine}, {secs:.1}s)"
                            )
                        }
                    ),
                    None => println!(
                        "\n{}",
                        tr!(
                            "✕ 编译失败（{engine}，{secs:.1}s）",
                            "✕ Compilation failed ({engine}, {secs:.1}s)"
                        )
                    ),
                }
            }
            Ok(if out["success"] == true {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            })
        }
        Cmd::BibImport { id } => {
            let registry = ym_commands::Registry::with_builtins();
            let mut ws = ym_commands::Workspace::default();
            ws.open(root)?;
            if let Some(m) = &cli.main {
                ws.set_main(m, false)?;
            }
            let out = registry.execute(&mut ws, "bib.import", serde_json::json!({ "id": id }))?;
            if cli.json {
                print_json(&out)?;
            } else if out["duplicate"] == true {
                println!(
                    "{}",
                    tr!(
                        "文献库中已有这篇：{k}",
                        "Already in the bibliography: {k}",
                        k = out["key"].as_str().unwrap_or("")
                    )
                );
            } else {
                println!(
                    "{}",
                    tr!(
                        "✓ 已导入 {k} → {f}",
                        "✓ Imported {k} → {f}",
                        k = out["key"].as_str().unwrap_or(""),
                        f = out["file"].as_str().unwrap_or("")
                    )
                );
                if out["needs_bibliography"] == true {
                    println!(
                        "{}",
                        tr!(
                            "提示：新建了 .bib 文件，请在正文中用 \\bibliography 引入",
                            "Note: a new .bib file was created; add it to the paper with \\bibliography"
                        )
                    );
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn print_outline(sections: &[Section], depth: usize) {
    for s in sections {
        let label = s
            .label
            .as_deref()
            .map(|l| format!("  [{l}]"))
            .unwrap_or_default();
        println!(
            "{}{}  {}{}  ({}:{})",
            "  ".repeat(depth),
            s.title,
            tr!("{n} 字", "{n} words", n = s.words),
            label,
            s.span.file.display(),
            s.span.line
        );
        print_outline(&s.children, depth + 1);
    }
}

fn format_issue(issue: &Issue) -> String {
    let severity = match issue.severity() {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let loc = match issue.location() {
        (Some(f), Some(l)) => format!("{}:{l}: ", f.display()),
        (Some(f), None) => format!("{}: ", f.display()),
        _ => String::new(),
    };
    format!("{loc}{severity}[{}] {}", issue.code(), issue.message())
}

fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

#[derive(Serialize)]
struct OutlineJson<'a> {
    main: &'a Path,
    total_words: u32,
    front_words: u32,
    outline: &'a [Section],
}

#[derive(Serialize)]
struct CheckJson<'a> {
    errors: usize,
    warnings: usize,
    issues: Vec<IssueJson<'a>>,
}

#[derive(Serialize)]
struct IssueJson<'a> {
    severity: Severity,
    message: String,
    file: Option<&'a Path>,
    line: Option<u32>,
    #[serde(flatten)]
    detail: &'a Issue,
}

impl<'a> From<&'a Issue> for IssueJson<'a> {
    fn from(issue: &'a Issue) -> Self {
        let (file, line) = issue.location();
        Self {
            severity: issue.severity(),
            message: issue.message(),
            file,
            line,
            detail: issue,
        }
    }
}
