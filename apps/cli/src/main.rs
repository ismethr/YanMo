use anyhow::{bail, Context};
use clap::{Parser, Subcommand};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use ym_check::{Issue, Severity};
use ym_model::Section;

#[derive(Parser)]
#[command(name = "ym", version, about = "研墨 YanMo 命令行")]
struct Cli {
    /// 项目根目录，默认当前目录
    #[arg(short = 'C', long, default_value = ".")]
    project: PathBuf,
    /// 主文件（相对项目根目录），默认自动识别
    #[arg(long)]
    main: Option<PathBuf>,
    /// 以 JSON 输出，便于脚本与 Agent 消费
    #[arg(long)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 编译论文
    Compile,
    /// 打印大纲与字数
    Outline,
    /// 检查引用、文献、图片问题；有错误时退出码为 1（可用于 CI）
    Check {
        /// 警告也视为失败
        #[arg(long)]
        strict: bool,
    },
    /// 通过 DOI / arXiv ID 导入文献
    BibImport { id: String },
    /// 执行命令注册表中的任意命令，例如 `ym exec project.index`；不带 ID 时列出所有命令
    Exec {
        id: Option<String>,
        /// JSON 参数，例如 '{"path":"sections/intro.tex"}'
        params: Option<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("错误：{e:#}");
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
                format!(
                    "在 {} 中找不到含 \\documentclass 的主文件，可用 --main 指定",
                    root.display()
                )
            })?,
        };
        ym_latex::index_project(root, &main)
            .with_context(|| format!("无法读取主文件 {}", root.join(&main).display()))
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
                    "{}  共 {} 字",
                    project.main.display(),
                    project.total_words()
                );
                if project.front_words > 0 {
                    println!("  （前置内容）  {} 字", project.front_words);
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
                    println!("✓ 未发现问题");
                } else {
                    println!("\n{errors} 个错误，{warnings} 个警告");
                }
            }
            let failed = errors > 0 || (strict && warnings > 0);
            Ok(if failed {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            })
        }
        Cmd::Exec { id, params } => {
            let registry = ym_commands::Registry::with_builtins();
            let Some(id) = id else {
                for m in registry.list() {
                    let mark = if m.mutating { "  [修改文件]" } else { "" };
                    println!("{:<16} {}{mark}", m.id, m.title);
                }
                return Ok(ExitCode::SUCCESS);
            };
            let mut ws = ym_commands::Workspace::default();
            ws.open(root)?;
            let params = match params {
                Some(p) => serde_json::from_str(&p).context("参数不是合法 JSON")?,
                None => serde_json::Value::Null,
            };
            print_json(&registry.execute(&mut ws, &id, params)?)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Compile => bail!("ym compile 尚未实现（M1）"),
        Cmd::BibImport { .. } => bail!("ym bib-import 尚未实现（M2 后续）"),
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
            "{}{}  {} 字{}  ({}:{})",
            "  ".repeat(depth),
            s.title,
            s.words,
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
