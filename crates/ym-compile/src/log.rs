//! 把 TeX 日志解析为结构化诊断信息。
//!
//! 日志里没有直接标明消息来自哪个文件：TeX 打开文件时输出 `(文件名`，关闭时输出 `)`。
//! 这里跟踪这个括号栈来确定当前文件。同时兼容 `-file-line-error` 产生的 `文件:行: 消息`。

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    #[serde(rename = "badbox")]
    BadBox,
}

/// 日志中的一条消息；`file` 是 TeX 看到的文件名（相对编译目录，可能省略 `.tex`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RawDiagnostic {
    pub severity: Severity,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
}

/// TeX 默认在 79 个字符处折行。
const MAX_PRINT_LINE: usize = 79;

/// 这些提示是汇总或噪音，单独列出没有意义。
const IGNORED: &[&str] = &[
    "There were undefined references",
    "There were multiply-defined labels",
    "Rerun to get",
    "does not contain requested Script",
    "Label(s) may have changed",
];

fn unwrap_lines(log: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut joining = false;
    for line in log.lines() {
        // 恰好 79 个字符的行也可能本身就完整：下一行是 TeX 的结构化提示时不拼接
        let starts_new = line.starts_with('<') || line.starts_with('!') || line.starts_with("l.");
        if joining && !starts_new {
            out.last_mut()
                .expect("joining implies a previous line")
                .push_str(line);
        } else {
            out.push(line.to_owned());
        }
        joining = line.chars().count() == MAX_PRINT_LINE;
    }
    out
}

/// 解析 `文件:行: 消息`（-file-line-error 格式）。
fn file_line_error(line: &str) -> Option<(String, u32, String)> {
    let mut parts = line.splitn(3, ':');
    let file = parts.next()?;
    let num = parts.next()?;
    let msg = parts.next()?.strip_prefix(' ')?;
    let looks_like_path = !file.is_empty()
        && !file.contains(' ')
        && (file.contains('/') || file.contains('.'))
        && !file.starts_with('(');
    let num: u32 = num.parse().ok()?;
    looks_like_path.then(|| (file.to_owned(), num, msg.to_owned()))
}

fn input_line(msg: &str) -> Option<u32> {
    let i = msg.rfind("input line ")?;
    msg[i + "input line ".len()..]
        .trim_end_matches('.')
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// 跟踪括号文件栈。文件名判定：含 `.` 或 `/`，或者 `(` 后的词一直延续到行尾。
struct FileStack(Vec<Option<String>>);

impl FileStack {
    fn feed(&mut self, line: &str) {
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            match chars[i] {
                '(' => {
                    let start = i + 1;
                    let mut j = start;
                    while j < chars.len() && !matches!(chars[j], ' ' | '(' | ')' | '"') {
                        j += 1;
                    }
                    let token: String = chars[start..j].iter().collect();
                    let at_eol = j == chars.len();
                    let is_file =
                        !token.is_empty() && (token.contains('.') || token.contains('/') || at_eol);
                    self.0.push(is_file.then_some(token));
                    i = j;
                    continue;
                }
                ')' => {
                    self.0.pop();
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn current(&self) -> Option<String> {
        self.0.iter().rev().find_map(|f| f.clone())
    }
}

/// 解析完整的 .log 文本。
pub fn parse_log(log: &str) -> Vec<RawDiagnostic> {
    let lines = unwrap_lines(log);
    let mut stack = FileStack(Vec::new());
    let mut out: Vec<RawDiagnostic> = Vec::new();
    let mut push = |d: RawDiagnostic| {
        if !IGNORED.iter().any(|p| d.message.contains(p)) && !out.contains(&d) {
            out.push(d);
        }
    };

    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];

        if let Some((file, num, msg)) = file_line_error(line) {
            push(RawDiagnostic {
                severity: Severity::Error,
                file: Some(file),
                line: Some(num),
                message: msg.trim().to_owned(),
            });
            i += 1;
            continue;
        }

        if let Some(msg) = line.strip_prefix("! ") {
            // 错误后几行内的 `l.<行号>` 指出出错位置
            let num = lines[i + 1..lines.len().min(i + 12)]
                .iter()
                .find_map(|l| l.strip_prefix("l.")?.split(' ').next()?.parse().ok());
            push(RawDiagnostic {
                severity: Severity::Error,
                file: stack.current(),
                line: num,
                message: msg.trim().to_owned(),
            });
            i += 1;
            continue;
        }

        let warning = ["LaTeX Warning: ", "Package ", "Class "]
            .iter()
            .find_map(|p| line.strip_prefix(p).map(|rest| (p, rest)))
            .and_then(|(p, rest)| {
                if *p == "LaTeX Warning: " {
                    Some(rest.to_owned())
                } else {
                    let (_, msg) = rest.split_once(" Warning: ")?;
                    Some(msg.to_owned())
                }
            });
        if let Some(first) = warning {
            // 续行以 `(包名)` 开头，或是以空格开头的折行，直到空行
            let mut msg = first;
            let mut j = i + 1;
            while j < lines.len() && !lines[j].trim().is_empty() {
                let l = &lines[j];
                let cont = if l.starts_with('(') {
                    l.split_once(')').map(|(_, r)| r).unwrap_or("")
                } else if l.starts_with(' ') && !l.trim_start().starts_with(')') {
                    l.as_str()
                } else {
                    break;
                };
                msg.push(' ');
                msg.push_str(cont.trim());
                j += 1;
            }
            let msg = msg.split_whitespace().collect::<Vec<_>>().join(" ");
            push(RawDiagnostic {
                severity: Severity::Warning,
                file: stack.current(),
                line: input_line(&msg),
                message: msg,
            });
            i = j;
            continue;
        }

        if line.starts_with("Overfull \\") || line.starts_with("Underfull \\") {
            let num = line
                .split_once("at lines ")
                .or_else(|| line.split_once("at line "))
                .and_then(|(_, r)| r.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok());
            push(RawDiagnostic {
                severity: Severity::BadBox,
                file: stack.current(),
                line: num,
                message: line.trim().to_owned(),
            });
        }

        stack.feed(line);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = r"This is XeTeX, Version 3.141592653
(main.tex
LaTeX2e <2025-06-01>
(/usr/share/texmf/tex/latex/base/article.cls
Document Class: article 2024/06/29 v1.4n Standard LaTeX document class
(size10.clo))
(sections/intro

LaTeX Warning: Reference `fig:missing' on page 1 undefined on input line 3.


Package natbib Warning: Citation `nobody2099' on page 1 undefined on input line
 4.

) (sections/method
! Unable to load picture or PDF file 'figures/arch.pdf'.
<to be read again>
                   }
l.4 ...phics[width=.8\linewidth]{figures/arch.pdf}

Overfull \hbox (12.0pt too wide) in paragraph at lines 7--9
)
LaTeX Warning: There were undefined references.
./sections/intro.tex:5: Undefined control sequence.
 )";

    #[test]
    fn parses_errors_warnings_and_files() {
        let d = parse_log(LOG);
        let summary: Vec<_> = d
            .iter()
            .map(|d| (d.severity, d.file.as_deref(), d.line))
            .collect();
        assert_eq!(
            summary,
            [
                (Severity::Warning, Some("sections/intro"), Some(3)),
                (Severity::Warning, Some("sections/intro"), Some(4)),
                (Severity::Error, Some("sections/method"), Some(4)),
                (Severity::BadBox, Some("sections/method"), Some(7)),
                (Severity::Error, Some("./sections/intro.tex"), Some(5)),
            ],
            "{d:#?}"
        );
        assert!(d[0].message.starts_with("Reference `fig:missing'"));
    }

    #[test]
    fn exact_width_error_is_not_joined_with_context() {
        let err = format!(
            "./sections/method.tex:4: {}",
            "y".repeat(MAX_PRINT_LINE - 25)
        );
        assert_eq!(err.chars().count(), MAX_PRINT_LINE);
        let d = parse_log(&format!("{err}\n<to be read again>\n"));
        assert!(!d[0].message.contains("<to be read again>"));
    }

    #[test]
    fn wrapped_lines_are_joined() {
        let long = format!("LaTeX Warning: {}", "x".repeat(MAX_PRINT_LINE - 15));
        assert_eq!(long.chars().count(), MAX_PRINT_LINE);
        let log = format!("(main.tex\n{long}\non input line 12.\n)");
        let d = parse_log(&log);
        assert_eq!(d[0].line, Some(12));
    }
}
