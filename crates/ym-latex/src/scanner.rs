//! 单文件扫描器。
//!
//! 按字节扫描（LaTeX 的特殊字符都是 ASCII），`pos` 始终停在 UTF-8 字符边界上。
//! 容错策略与 LaTeX 自身一致：参数不能跨空行（空行即 `\par`），参数不完整的命令直接忽略。

use std::path::{Path, PathBuf};
use ym_model::{Asset, AssetKind, SectionLevel, SourceSpan};

use crate::{text, FileIndex, FlowItem, Heading, Located};

/// 内容原样跳过、不解析的环境。
const VERBATIM_ENVS: &[&str] = &[
    "verbatim",
    "verbatim*",
    "Verbatim",
    "BVerbatim",
    "lstlisting",
    "minted",
    "comment",
];

/// 数学环境：其中的文字不计字数。
const MATH_ENVS: &[&str] = &[
    "equation",
    "align",
    "gather",
    "multline",
    "eqnarray",
    "flalign",
    "alignat",
    "math",
    "displaymath",
    "split",
    "cases",
];

/// 参数里没有正文的命令：跳过其全部参数。
const NO_TEXT_COMMANDS: &[&str] = &[
    "usepackage",
    "RequirePackage",
    "bibliographystyle",
    "vspace",
    "hspace",
    "setlength",
    "addtolength",
    "setcounter",
    "addtocounter",
    "newcommand",
    "renewcommand",
    "providecommand",
    "newenvironment",
    "renewenvironment",
    "DeclareMathOperator",
    "newtheorem",
    "pagestyle",
    "thispagestyle",
    "url",
    "hypersetup",
    "includepdf",
    "definecolor",
    "color",
    "geometry",
    "captionsetup",
    "crefname",
    "Crefname",
];

const REF_COMMANDS: &[&str] = &[
    "ref",
    "eqref",
    "pageref",
    "autoref",
    "Autoref",
    "cref",
    "Cref",
    "cpageref",
    "Cpageref",
    "nameref",
    "vref",
    "Vref",
    "subref",
    "labelcref",
    "crefrange",
    "Crefrange",
    "cpagerefrange",
    "Cpagerefrange",
];

/// `\citep` 这类以 cite 开头的命令（以 cite/cites 结尾的另行判断）。
const CITE_PREFIXED: &[&str] = &[
    "citep",
    "citet",
    "citealp",
    "citealt",
    "citeauthor",
    "citeyear",
    "citeyearpar",
    "citetitle",
    "citenum",
    "citeurl",
    "citedate",
    "Citep",
    "Citet",
    "Citealp",
    "Citealt",
    "Citeauthor",
];

fn is_cite_command(name: &str) -> bool {
    name.ends_with("cite") || name.ends_with("cites") || CITE_PREFIXED.contains(&name)
}

fn section_level(name: &str) -> Option<SectionLevel> {
    Some(match name {
        "part" => SectionLevel::Part,
        "chapter" => SectionLevel::Chapter,
        "section" => SectionLevel::Section,
        "subsection" => SectionLevel::Subsection,
        "subsubsection" => SectionLevel::Subsubsection,
        "paragraph" => SectionLevel::Paragraph,
        _ => return None,
    })
}

/// 可被引用的资产环境。带星号的数学环境不编号，不算资产。
fn asset_kind(env: &str) -> Option<AssetKind> {
    let base = env.trim_end_matches('*');
    Some(match base {
        "figure" | "subfigure" | "wrapfigure" => AssetKind::Figure,
        "table" | "subtable" | "wraptable" => AssetKind::Table,
        "algorithm" => AssetKind::Algorithm,
        "listing" => AssetKind::Listing,
        _ if MATH_ENVS.contains(&base) && !env.ends_with('*') => AssetKind::Equation,
        _ => return None,
    })
}

/// `\begin{env}` 之后（可选参数之外）需要跳过的必选参数个数。
fn env_mandatory_args(env: &str) -> usize {
    match env {
        "tabular" | "array" | "longtable" | "minipage" | "subfigure" | "subtable" | "multicols"
        | "thebibliography" | "alignat" | "alignat*" => 1,
        "tabular*" | "tabularx" | "tabulary" | "wrapfigure" | "wraptable" => 2,
        _ => 0,
    }
}

struct Frame {
    name: String,
    asset: Option<Asset>,
    math: bool,
}

pub(crate) struct Parser<'a> {
    src: &'a str,
    b: &'a [u8],
    pos: usize,
    path: PathBuf,
    line_starts: Vec<usize>,
    out: FileIndex,
    frames: Vec<Frame>,
    /// 尚未写入 flow 的字数
    words: u32,
    /// 是否在正文中（`\documentclass` 与 `\begin{document}` 之间为导言区）
    counting: bool,
    /// `$...$`、`\(...\)`、`\[...\]` 内
    inline_math: bool,
    /// 刚解析完章节标题，紧随其后的 `\label` 归属该章节
    heading_label_open: bool,
}

fn utf8_len(lead: u8) -> usize {
    match lead {
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

impl<'a> Parser<'a> {
    pub(crate) fn new(path: &Path, src: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            src.bytes()
                .enumerate()
                .filter(|&(_, c)| c == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self {
            src,
            b: src.as_bytes(),
            pos: 0,
            path: path.to_path_buf(),
            line_starts,
            out: FileIndex {
                path: path.to_path_buf(),
                ..Default::default()
            },
            frames: Vec::new(),
            words: 0,
            counting: true,
            inline_math: false,
            heading_label_open: false,
        }
    }

    pub(crate) fn run(mut self) -> FileIndex {
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                b'%' => self.skip_comment(),
                b'\\' => self.command(),
                b'$' => {
                    self.pos += if self.peek_at(1) == Some(b'$') { 2 } else { 1 };
                    self.inline_math = !self.inline_math;
                    self.heading_label_open = false;
                }
                b'{' | b'}' => self.pos += 1,
                _ => self.text_run(),
            }
        }
        while let Some(frame) = self.frames.pop() {
            self.finish_frame(frame, self.b.len());
        }
        self.flush_words();
        self.out.assets.sort_by_key(|a| a.span.start);
        self.out
    }

    // ---------- 基础工具 ----------

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.b.get(self.pos + offset).copied()
    }

    fn bump_char(&mut self) {
        self.pos = (self.pos + utf8_len(self.b[self.pos])).min(self.b.len());
    }

    fn span(&self, start: usize, end: usize) -> SourceSpan {
        let line = self.line_starts.partition_point(|&s| s <= start);
        SourceSpan {
            file: self.path.clone(),
            start,
            end,
            line: line as u32,
        }
    }

    fn skip_comment(&mut self) {
        while self.pos < self.b.len() && self.b[self.pos] != b'\n' {
            self.pos += 1;
        }
    }

    /// 当前位置是 `\n`，且下一行为空白行。
    fn at_blank_line(&self) -> bool {
        self.b[self.pos + 1..]
            .iter()
            .find(|c| !matches!(c, b' ' | b'\t' | b'\r'))
            .is_some_and(|&c| c == b'\n')
    }

    fn text_run(&mut self) {
        let start = self.pos;
        while self.pos < self.b.len()
            && !matches!(self.b[self.pos], b'%' | b'\\' | b'$' | b'{' | b'}')
        {
            self.pos += 1;
        }
        let run = &self.src[start..self.pos];
        if !run.trim().is_empty() {
            self.heading_label_open = false;
        }
        if self.inline_math && run.split('\n').skip(1).any(|l| l.trim().is_empty()) {
            // 未闭合的行内公式在段落结束处恢复
            self.inline_math = false;
            return;
        }
        if self.counting && !self.inline_math && !self.frames.iter().any(|f| f.math) {
            self.words += text::count_words(run);
        }
    }

    fn flush_words(&mut self) {
        if self.words > 0 {
            self.out.flow.push(FlowItem::Words(self.words));
            self.words = 0;
        }
    }

    // ---------- 参数读取 ----------

    /// 跳过参数前的空白与注释，但不跨越空行。
    fn skip_arg_ws(&mut self) {
        while self.pos < self.b.len() {
            match self.b[self.pos] {
                b' ' | b'\t' | b'\r' => self.pos += 1,
                b'\n' if !self.at_blank_line() => self.pos += 1,
                b'%' => self.skip_comment(),
                _ => return,
            }
        }
    }

    /// 读取 `open ... close` 包围的参数，返回内容的字节范围；失败时不移动位置。
    fn read_delimited(&mut self, open: u8, close: u8) -> Option<(usize, usize)> {
        let save = self.pos;
        self.skip_arg_ws();
        if self.b.get(self.pos) != Some(&open) {
            self.pos = save;
            return None;
        }
        self.pos += 1;
        let start = self.pos;
        let mut depth = 0usize;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            match c {
                b'\\' => {
                    self.pos += 1;
                    if self.pos < self.b.len() {
                        self.bump_char();
                    }
                    continue;
                }
                b'%' => {
                    self.skip_comment();
                    continue;
                }
                b'\n' if self.at_blank_line() => break,
                _ if c == close && depth == 0 => {
                    self.pos += 1;
                    return Some((start, self.pos - 1));
                }
                b'{' => depth += 1,
                b'}' if depth == 0 => break,
                b'}' => depth -= 1,
                _ => {}
            }
            self.bump_char();
        }
        self.pos = save;
        None
    }

    fn group(&mut self) -> Option<(usize, usize)> {
        self.read_delimited(b'{', b'}')
    }

    fn opt(&mut self) -> Option<(usize, usize)> {
        self.read_delimited(b'[', b']')
    }

    fn group_str(&mut self) -> Option<(&'a str, usize, usize)> {
        let src = self.src;
        self.group().map(|(s, e)| (src[s..e].trim(), s, e))
    }

    fn skip_all_args(&mut self) {
        while self.opt().is_some() || self.group().is_some() {}
    }

    /// 把逗号分隔的参数内容拆成多个带精确位置的名字。
    fn split_names(&self, (start, end): (usize, usize)) -> Vec<Located> {
        let mut result = Vec::new();
        let mut offset = start;
        for piece in self.src[start..end].split(',') {
            let lead = piece.len() - piece.trim_start().len();
            let name = piece.trim();
            if !name.is_empty() {
                let s = offset + lead;
                result.push(Located {
                    name: name.to_owned(),
                    span: self.span(s, s + name.len()),
                });
            }
            offset += piece.len() + 1;
        }
        result
    }

    fn innermost_asset(&mut self) -> Option<&mut Asset> {
        self.frames.iter_mut().rev().find_map(|f| f.asset.as_mut())
    }

    // ---------- 命令 ----------

    fn command(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let Some(c) = self.b.get(self.pos).copied() else {
            return;
        };
        if !c.is_ascii_alphabetic() {
            match c {
                b'(' | b'[' => self.inline_math = true,
                b')' | b']' => self.inline_math = false,
                _ => {}
            }
            self.bump_char();
            return;
        }
        let name_start = self.pos;
        while self.pos < self.b.len() && self.b[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        let src = self.src;
        let name = &src[name_start..self.pos];
        let starred = self.b.get(self.pos) == Some(&b'*');
        if starred {
            self.pos += 1;
        }
        if name != "label" {
            self.heading_label_open = false;
        }
        if self.counting
            && matches!(
                name,
                "appendix"
                    | "bibliography"
                    | "bibliographystyle"
                    | "printbibliography"
                    | "backmatter"
            )
        {
            self.out.breaks.push(start);
        }

        match name {
            "verb" => self.skip_verb(),
            "lstinline" | "mintinline" => {
                self.opt();
                if name == "mintinline" {
                    self.group();
                }
                self.skip_verb();
            }
            "iffalse" => self.skip_iffalse(),
            "documentclass" => {
                self.out.is_root_document = true;
                self.counting = false;
                self.skip_all_args();
            }
            "begin" => self.begin(start),
            "end" => self.end(start),
            "label" => self.label(),
            "caption" => self.caption(),
            "includegraphics" => self.includegraphics(start),
            "graphicspath" => self.graphicspath(),
            "input" | "include" | "subfile" => self.include(start),
            "import" | "subimport" | "inputfrom" | "subinputfrom" | "includefrom"
            | "subincludefrom" => self.import(start),
            "bibliography" => {
                if let Some(r) = self.group() {
                    let names = self.split_names(r);
                    self.out.bib_files.extend(names);
                }
            }
            "addbibresource" | "addglobalbib" => {
                self.opt();
                if let Some(r) = self.group() {
                    let names = self.split_names(r);
                    self.out.bib_files.extend(names);
                }
            }
            "bibitem" => {
                self.opt();
                if let Some(r) = self.group() {
                    let names = self.split_names(r);
                    self.out.bibitems.extend(names);
                }
            }
            "href" | "textcolor" | "colorbox" => {
                self.group(); // 第一个参数是 URL / 颜色，第二个才是正文
            }
            _ if section_level(name).is_some() => self.heading(name, starred, start),
            _ if REF_COMMANDS.contains(&name) => self.refs(name),
            _ if is_cite_command(name) => self.cites(name),
            _ if NO_TEXT_COMMANDS.contains(&name) => self.skip_all_args(),
            _ => {}
        }
    }

    /// `\verb|...|`：参数以任意字符定界，不跨行。也兼容 `\lstinline{...}`。
    fn skip_verb(&mut self) {
        let Some(delim) = self.b.get(self.pos).copied() else {
            return;
        };
        if delim == b'{' {
            self.group();
            return;
        }
        if delim.is_ascii_whitespace() || delim.is_ascii_alphabetic() {
            return;
        }
        self.bump_char();
        while self.pos < self.b.len() && !matches!(self.b[self.pos], b'\n') {
            if self.b[self.pos] == delim {
                self.pos += 1;
                return;
            }
            self.bump_char();
        }
    }

    fn skip_iffalse(&mut self) {
        let mut from = self.pos;
        while let Some(i) = self.src[from..].find("\\fi") {
            let after = from + i + 3;
            if !self.b.get(after).is_some_and(|c| c.is_ascii_alphabetic()) {
                self.pos = after;
                return;
            }
            from = after;
        }
        self.pos = self.b.len();
    }

    fn heading(&mut self, name: &str, starred: bool, start: usize) {
        if !self.counting {
            return; // 导言区里的章节命令多半在宏定义中
        }
        self.opt();
        let Some((title, _, _)) = self.group_str() else {
            return;
        };
        self.flush_words();
        self.out.headings.push(Heading {
            level: section_level(name).expect("checked by caller"),
            title: text::plain(title),
            label: None,
            starred,
            span: self.span(start, self.pos),
        });
        self.out
            .flow
            .push(FlowItem::Heading(self.out.headings.len() - 1));
        self.heading_label_open = true;
    }

    fn label(&mut self) {
        let Some((name, s, e)) = self.group_str() else {
            return;
        };
        if name.is_empty() {
            return;
        }
        let lead = self.src[s..e].len() - self.src[s..e].trim_start().len();
        let span = self.span(s + lead, s + lead + name.len());
        self.out.labels.push(Located {
            name: name.to_owned(),
            span,
        });

        if let Some(asset) = self.innermost_asset() {
            asset.label.get_or_insert_with(|| name.to_owned());
        } else if self.heading_label_open {
            if let Some(h) = self.out.headings.last_mut() {
                h.label.get_or_insert_with(|| name.to_owned());
            }
        }
        self.heading_label_open = false;
    }

    /// 图注：提取纯文本后，回到参数内部继续扫描，以收集其中的 `\label`、`\cite` 和字数。
    fn caption(&mut self) {
        self.opt();
        let Some((caption, s, _)) = self.group_str() else {
            return;
        };
        let caption = text::plain(caption);
        if let Some(asset) = self.innermost_asset() {
            asset.caption.get_or_insert(caption);
        }
        self.pos = s;
    }

    fn refs(&mut self, name: &str) {
        let count = if name.ends_with("range") { 2 } else { 1 };
        for _ in 0..count {
            if let Some(r) = self.group() {
                let names = self.split_names(r);
                self.out.refs.extend(names);
            }
        }
    }

    fn cites(&mut self, name: &str) {
        let multi = name.ends_with("cites");
        loop {
            self.opt();
            self.opt();
            let Some(r) = self.group() else {
                break;
            };
            for key in self.split_names(r) {
                if key.name == "*" {
                    self.out.nocite_all = true;
                } else {
                    self.out.cites.push(key);
                }
            }
            if !multi {
                break;
            }
        }
    }

    fn includegraphics(&mut self, start: usize) {
        self.opt();
        let Some((path, _, _)) = self.group_str() else {
            return;
        };
        if path.is_empty() {
            return;
        }
        let span = self.span(start, self.pos);
        self.out.graphics.push(Located {
            name: path.to_owned(),
            span,
        });
        if let Some(asset) = self.innermost_asset() {
            asset.files.push(PathBuf::from(path));
        }
    }

    fn graphicspath(&mut self) {
        let Some((inner, _, _)) = self.group_str() else {
            return;
        };
        let mut depth = 0;
        let mut current = String::new();
        for c in inner.chars() {
            match c {
                '{' => {
                    depth += 1;
                    if depth == 1 {
                        current.clear();
                        continue;
                    }
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let dir = current.trim();
                        if !dir.is_empty() {
                            self.out.graphics_paths.push(dir.to_owned());
                        }
                        continue;
                    }
                }
                _ => {}
            }
            if depth >= 1 {
                current.push(c);
            }
        }
    }

    fn include(&mut self, start: usize) {
        let target = match self.group_str() {
            Some((t, _, _)) => t.to_owned(),
            None => {
                // TeX 原语写法：\input file
                while self.peek_at(0) == Some(b' ') {
                    self.pos += 1;
                }
                let s = self.pos;
                while self.pos < self.b.len()
                    && !self.b[self.pos].is_ascii_whitespace()
                    && !matches!(self.b[self.pos], b'%' | b'\\' | b'{' | b'}' | b'$')
                {
                    self.pos += 1;
                }
                self.src[s..self.pos].to_owned()
            }
        };
        self.push_include(target, start);
    }

    fn import(&mut self, start: usize) {
        let Some((dir, _, _)) = self.group_str() else {
            return;
        };
        let Some((file, _, _)) = self.group_str() else {
            return;
        };
        let target = Path::new(dir).join(file).to_string_lossy().into_owned();
        self.push_include(target, start);
    }

    fn push_include(&mut self, target: String, start: usize) {
        if target.is_empty() {
            return;
        }
        self.flush_words();
        let span = self.span(start, self.pos);
        self.out.includes.push(Located { name: target, span });
        let i = self.out.includes.len() - 1;
        self.out.flow.push(if self.counting {
            FlowItem::Include(i)
        } else {
            FlowItem::PreambleInclude(i)
        });
    }

    // ---------- 环境 ----------

    fn begin(&mut self, start: usize) {
        let Some((env, _, _)) = self.group_str() else {
            return;
        };
        if env == "thebibliography" && self.counting {
            self.out.breaks.push(start);
        }
        if VERBATIM_ENVS.contains(&env) {
            let end = format!("\\end{{{env}}}");
            self.pos = match self.src[self.pos..].find(&end) {
                Some(i) => self.pos + i + end.len(),
                None => self.b.len(),
            };
            return;
        }
        if env == "document" {
            self.counting = true;
            return;
        }

        let asset = asset_kind(env).map(|kind| Asset {
            kind,
            label: None,
            caption: None,
            files: Vec::new(),
            span: self.span(start, start),
            references: Vec::new(),
        });
        let math = MATH_ENVS.contains(&env.trim_end_matches('*'));
        if !math {
            self.opt(); // 如 figure 的 [htbp]
        }
        for _ in 0..env_mandatory_args(env) {
            self.group();
        }
        self.frames.push(Frame {
            name: env.to_owned(),
            asset,
            math,
        });
    }

    fn end(&mut self, start: usize) {
        let Some((env, _, _)) = self.group_str() else {
            return;
        };
        if env == "document" {
            self.out.breaks.push(start);
            self.flush_words();
            self.pos = self.b.len(); // \end{document} 之后的内容 LaTeX 不处理
            return;
        }
        // 容错：弹出到最近的同名环境，中间未闭合的环境一并关闭
        if let Some(i) = self.frames.iter().rposition(|f| f.name == env) {
            while self.frames.len() > i {
                let frame = self.frames.pop().expect("len > i");
                self.finish_frame(frame, self.pos);
            }
        }
    }

    fn finish_frame(&mut self, frame: Frame, end: usize) {
        if let Some(mut asset) = frame.asset {
            // 没有 label 的公式无法被引用，不列入资产
            if asset.kind == AssetKind::Equation && asset.label.is_none() {
                return;
            }
            asset.span.end = end;
            self.out.assets.push(asset);
        }
    }
}
