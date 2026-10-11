//! 文献管理：解析、写回（尽量保持原始格式）、在线导入。

pub mod import;
pub mod verify;

use std::collections::BTreeMap;
use std::path::Path;
use ym_model::{tr, BibEntry, SourceSpan};

#[derive(Debug, Clone, thiserror::Error)]
pub enum BibError {
    #[error("line {line}: {msg}")]
    Parse { line: u32, msg: String },
    #[error("network: {0}")]
    Network(String),
}

/// 容错解析结果：出错的条目跳过或部分保留，错误单独列出。
#[derive(Debug, Default)]
pub struct ParseOutput {
    pub entries: Vec<BibEntry>,
    pub errors: Vec<BibError>,
}

/// 解析 `.bib` 文件。字段值保留原始文本（去掉最外层的 `{}` 或 `""`）。
pub fn parse(path: &Path, src: &str) -> ParseOutput {
    BibParser {
        src,
        b: src.as_bytes(),
        pos: 0,
        path,
        out: ParseOutput::default(),
    }
    .run()
}

struct BibParser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    path: &'a Path,
    out: ParseOutput,
}

impl BibParser<'_> {
    fn run(mut self) -> ParseOutput {
        while let Some(off) = self.src[self.pos..].find('@') {
            let at = self.pos + off;
            self.pos = at + 1;
            self.entry(at);
        }
        self.out
    }

    fn line(&self, at: usize) -> u32 {
        self.src[..at.min(self.src.len())].matches('\n').count() as u32 + 1
    }

    fn error(&mut self, at: usize, msg: &str) {
        let line = self.line(at);
        self.out.errors.push(BibError::Parse {
            line,
            msg: msg.to_owned(),
        });
    }

    fn skip_ws(&mut self) {
        while self.pos < self.b.len() && self.b[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn take_while(&mut self, pred: impl Fn(u8) -> bool) -> &str {
        let start = self.pos;
        while self.pos < self.b.len() && pred(self.b[self.pos]) {
            self.pos += 1;
        }
        &self.src[start..self.pos]
    }

    fn entry(&mut self, at: usize) {
        let ty = self
            .take_while(|c| c.is_ascii_alphanumeric() || c == b'_')
            .to_ascii_lowercase();
        if ty.is_empty() {
            return; // 普通文本里的 @
        }
        self.skip_ws();
        let close = match self.b.get(self.pos) {
            Some(b'{') => b'}',
            Some(b'(') => b')',
            _ => {
                return self.error(
                    at,
                    &tr!(
                        "条目类型后缺少 {{ 或 (",
                        "expected {{ or ( after the entry type"
                    ),
                )
            }
        };
        self.pos += 1;
        if matches!(ty.as_str(), "comment" | "preamble" | "string") {
            self.skip_balanced(close);
            return;
        }

        self.skip_ws();
        let key = self
            .take_while(|c| c != b',' && c != close && !c.is_ascii_whitespace())
            .to_owned();
        let mut fields = BTreeMap::new();
        loop {
            self.skip_ws();
            match self.b.get(self.pos) {
                None => {
                    self.error(at, &tr!("条目 {key} 未闭合", "entry {key} is not closed"));
                    break;
                }
                Some(&c) if c == close => {
                    self.pos += 1;
                    break;
                }
                Some(b',') => self.pos += 1,
                Some(b'@') => {
                    // 缺少右括号，下一个条目已开始：在此恢复
                    self.error(
                        at,
                        &tr!(
                            "条目 {key} 缺少结尾的 {c}",
                            "entry {key} is missing the closing {c}",
                            c = close as char
                        ),
                    );
                    break;
                }
                Some(_) => {
                    let name = self
                        .take_while(|c| c.is_ascii_alphanumeric() || b"_-:.".contains(&c))
                        .to_ascii_lowercase();
                    if name.is_empty() {
                        let here = self.pos;
                        self.error(
                            here,
                            &tr!(
                                "条目 {key} 中有无法识别的字符",
                                "unexpected character in entry {key}"
                            ),
                        );
                        self.pos += 1;
                        while !self.src.is_char_boundary(self.pos) {
                            self.pos += 1;
                        }
                        continue;
                    }
                    self.skip_ws();
                    if self.b.get(self.pos) != Some(&b'=') {
                        let here = self.pos;
                        self.error(
                            here,
                            &tr!("字段 {name} 后缺少 =", "expected = after field {name}"),
                        );
                        continue;
                    }
                    self.pos += 1;
                    let value = self.value(close);
                    fields.insert(name, value);
                }
            }
        }

        if key.is_empty() {
            self.error(at, &tr!("条目缺少引用 key", "entry has no citation key"));
            return;
        }
        let line = self.line(at);
        self.out.entries.push(BibEntry {
            key,
            entry_type: ty,
            fields,
            span: SourceSpan {
                file: self.path.to_path_buf(),
                start: at,
                end: self.pos,
                line,
            },
            citations: Vec::new(),
        });
    }

    /// 字段值：`{...}`、`"..."`、数字或 @string 宏，可用 `#` 连接。
    fn value(&mut self, close: u8) -> String {
        let mut value = String::new();
        loop {
            self.skip_ws();
            match self.b.get(self.pos) {
                Some(b'{') => {
                    self.pos += 1;
                    let start = self.pos;
                    let end = self.skip_balanced(b'}');
                    value.push_str(&self.src[start..end]);
                }
                Some(b'"') => {
                    self.pos += 1;
                    let start = self.pos;
                    let mut depth = 0usize;
                    while self.pos < self.b.len() {
                        match self.b[self.pos] {
                            b'\\' => self.pos += 1,
                            b'{' => depth += 1,
                            b'}' => depth = depth.saturating_sub(1),
                            b'"' if depth == 0 => break,
                            _ => {}
                        }
                        self.pos += 1;
                    }
                    let end = self.pos.min(self.b.len());
                    value.push_str(&self.src[start..end]);
                    self.pos = (self.pos + 1).min(self.b.len());
                }
                Some(_) => {
                    let token = self.take_while(|c| {
                        !c.is_ascii_whitespace() && c != b',' && c != b'#' && c != close
                    });
                    value.push_str(token);
                }
                None => {}
            }
            self.skip_ws();
            if self.b.get(self.pos) == Some(&b'#') {
                self.pos += 1;
            } else {
                return value;
            }
        }
    }

    fn next_line_starts_entry(&self) -> bool {
        let rest = self.src[self.pos + 1..].trim_start_matches([' ', '\t']);
        rest.starts_with('@') && rest[1..].starts_with(|c: char| c.is_ascii_alphabetic())
    }

    /// 从当前位置跳到与 `close` 匹配处之后，返回 `close` 的位置。
    fn skip_balanced(&mut self, close: u8) -> usize {
        let mut depth = 0usize;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            match c {
                b'\\' => {
                    self.pos += 2;
                    continue;
                }
                b'\n' if self.next_line_starts_entry() => {
                    // 括号未闭合：在下一个条目处恢复，避免吞掉后续所有条目
                    return self.pos;
                }
                b'{' => depth += 1,
                _ if c == close && depth == 0 => {
                    self.pos += 1;
                    return self.pos - 1;
                }
                b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            self.pos += 1;
        }
        self.pos = self.b.len();
        // 未闭合时退回到字符边界，保证切片安全
        let mut end = self.pos;
        while !self.src.is_char_boundary(end) {
            end -= 1;
        }
        end
    }
}

/// 生成引用 key：`<第一作者姓><年份><标题首个实词>`，如 `vaswani2017attention`。
pub fn make_key(entry: &BibEntry) -> String {
    const STOP: &[&str] = &[
        "a", "an", "the", "on", "of", "for", "in", "to", "and", "with",
    ];
    let ascii = |s: &str| -> String {
        s.chars()
            .filter(char::is_ascii_alphanumeric)
            .flat_map(char::to_lowercase)
            .collect()
    };
    let author = entry
        .field("author")
        .and_then(|a| a.split(" and ").next())
        .map(|first| match first.split_once(',') {
            Some((last, _)) => last,
            None => first.split_whitespace().last().unwrap_or(""),
        })
        .map(ascii)
        .unwrap_or_default();
    let year = entry.field("year").map(ascii).unwrap_or_default();
    let word = entry
        .field("title")
        .into_iter()
        .flat_map(str::split_whitespace)
        .map(ascii)
        .find(|w| !w.is_empty() && !STOP.contains(&w.as_str()))
        .unwrap_or_default();
    format!("{author}{year}{word}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_and_recovers() {
        let src = r#"@string{nips = "NeurIPS"}
@InProceedings{vaswani2017,
  title = {Attention Is {All} You Need},
  author = "Vaswani, Ashish and others",
  booktitle = nips # " 2017",
  year = 2017,
}
@article{broken,
  title = {Missing close brace
@misc(paren, note = {email a@b.c})
"#;
        let out = parse(Path::new("refs.bib"), src);
        let keys: Vec<_> = out.entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, ["vaswani2017", "broken", "paren"]);
        let v = &out.entries[0];
        assert_eq!(v.entry_type, "inproceedings");
        assert_eq!(v.field("title"), Some("Attention Is {All} You Need"));
        assert_eq!(v.field("year"), Some("2017"));
        assert_eq!(v.span.line, 2);
        assert_eq!(out.entries[2].field("note"), Some("email a@b.c"));
        assert!(!out.errors.is_empty());
        assert_eq!(make_key(v), "vaswani2017attention");
    }

    #[test]
    fn truncated_input_never_panics() {
        let src = "@article{k,\n  title = {标题 {嵌套}},\n  author = \"张 三\" # x,\n}";
        for (i, _) in src.char_indices() {
            let _ = parse(Path::new("r.bib"), &src[..i]);
        }
    }
    #[test]
    fn add_field_keeps_alignment() {
        let e = "@article{k,\n  title  = {T},\n  author = {A},\n  year   = {2020}\n}";
        assert_eq!(
            add_field(e, "doi", "10.1/x").unwrap(),
            "@article{k,\n  title  = {T},\n  author = {A},\n  year   = {2020},\n  doi    = {10.1/x}\n}"
        );
        let e = "@misc{k,\n    title = {T},\n}";
        assert_eq!(
            add_field(e, "doi", "10.1/x").unwrap(),
            "@misc{k,\n    title = {T},\n    doi = {10.1/x}\n}"
        );
        assert_eq!(
            add_field("@misc{k, title={T}}", "doi", "d").unwrap(),
            "@misc{k, title={T},\n  doi = {d}\n}"
        );
    }
}

/// 在条目文本末尾补一个字段（条目中还没有该字段时）。沿用条目已有的缩进与 `=` 对齐方式，
/// 只插入一行，其余内容不变。条目格式无法识别时返回 None。
pub fn add_field(entry: &str, field: &str, value: &str) -> Option<String> {
    let close = entry.trim_end().len().checked_sub(1)?;
    if !matches!(entry.as_bytes().get(close), Some(b'}' | b')')) {
        return None;
    }
    let body = &entry[..close];
    let lines: Vec<&str> = body.lines().skip(1).filter(|l| l.contains('=')).collect();
    // 缩进与 `=` 的列取自第一行字段
    let indent: String = lines
        .first()
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
        .unwrap_or_else(|| "  ".into());
    // 有字段名后跟多个空格再接 `=`，说明作者对齐了 `=`，新字段也对齐到同一列
    let aligned = lines.iter().any(|l| l.trim_start().contains("  ="));
    let width = if aligned {
        lines
            .first()
            .and_then(|l| l.trim_start().find('='))
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0)
    } else {
        0
    };
    let name = format!("{field:<width$}");
    // 最后一个字段若没有结尾逗号，先补上
    let trimmed = body.trim_end();
    let comma = if trimmed.ends_with(',') { "" } else { "," };
    let rest = &body[trimmed.len()..];
    let newline = if rest.contains('\n') { "" } else { "\n" };
    Some(format!(
        "{trimmed}{comma}\n{indent}{name} = {{{value}}}{newline}{rest}{}",
        &entry[close..]
    ))
}
