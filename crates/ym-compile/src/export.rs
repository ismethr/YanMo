//! 用 Pandoc 把 LaTeX 论文导出为 Word（.docx）。
//!
//! Pandoc 直接转换有几处不能用的地方，这里逐一补上：
//! - 交叉引用显示成 `[fig:x]`：从编译生成的 `.aux` 读出真实编号，经 Lua 过滤器替换，
//!   并给图表标题加上"Fig. n."/"Table n."，保证与 PDF 一致；
//! - Word 不能显示 PDF/EPS 图片：优先使用同名 PNG/JPG，否则用 Ghostscript（macOS 上退而用 sips）转换并缓存；
//! - 导言区的底层宏代码（`\ExplSyntaxOn…Off`、`\makeatletter…\makeatother`）会让 Pandoc 的解析错位，
//!   导致其后的图表环境全部识别失败：交给 Pandoc 前先剔除（经标准输入传入，不写临时文件）；
//! - `\title[...]{...}`（Elsevier 等文档类）带可选参数时 Pandoc 读不到标题：改写为 `\title{...}`；
//! - 章节编号、引用格式（项目中的 .csl）、Word 模板（`.yanmo/reference.docx`）。

use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime};

use crate::{CompileError, Toolchain, BUILD_DIR};

/// 导出产物目录，相对项目根目录。
pub const EXPORT_DIR: &str = ".yanmo/export";

#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub root: PathBuf,
    /// 相对 `root`
    pub main: PathBuf,
    /// 输出文件，相对 `root`；默认 `.yanmo/export/<主文件名>.docx`
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportOutput {
    /// 生成的 .docx，相对项目根目录
    pub path: String,
    pub duration_ms: u64,
    /// 交叉引用是否已替换为编号（需要先编译出 .aux）
    pub numbered_refs: bool,
    pub labels: usize,
    /// 由 PDF/EPS 转换为 PNG 的图片数
    pub figures_converted: usize,
    /// 无法转换、在 Word 中可能显示为空白的图片
    pub figures_missing: Vec<String>,
    /// 使用的引用样式（.csl）与 Word 模板，相对项目根目录
    pub csl: Option<String>,
    pub reference_doc: Option<String>,
    /// Pandoc 的警告（去重）
    pub warnings: Vec<String>,
}

/// 读取 `.aux` 中的 `\newlabel{label}{{编号}{页码}...}`。
pub fn read_aux_labels(aux: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in aux.lines() {
        let Some(rest) = line.strip_prefix("\\newlabel{") else {
            continue;
        };
        let Some((label, rest)) = rest.split_once('}') else {
            continue;
        };
        // cleveref 的辅助条目
        if label.ends_with("@cref") {
            continue;
        }
        let Some(rest) = rest.strip_prefix("{{") else {
            continue;
        };
        // 编号本身可能含花括号，按深度找到匹配的 `}`
        let mut depth = 0usize;
        let mut end = None;
        for (i, c) in rest.char_indices() {
            match c {
                '{' => depth += 1,
                '}' if depth == 0 => {
                    end = Some(i);
                    break;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        let Some(end) = end else { continue };
        let number = ym_latex::text::plain(&rest[..end]);
        if !number.is_empty() {
            out.insert(label.to_owned(), number);
        }
    }
    out
}

/// 预处理交给 Pandoc 的源码。
fn prepare_for_pandoc(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut skip_until: Option<&str> = None;
    for line in source.lines() {
        let t = line.trim_start();
        if let Some(end) = skip_until {
            if t.starts_with(end) {
                skip_until = None;
            }
            continue;
        }
        if t.starts_with("\\ExplSyntaxOn") && source.contains("\\ExplSyntaxOff") {
            skip_until = Some("\\ExplSyntaxOff");
            continue;
        }
        if t.starts_with("\\makeatletter") && source.contains("\\makeatother") {
            skip_until = Some("\\makeatother");
            continue;
        }
        if let Some(rest) = t.strip_prefix("\\title[") {
            if let Some(i) = rest.find("]{") {
                out.push_str("\\title{");
                out.push_str(&rest[i + 2..]);
                out.push('\n');
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// 提取第一个 `\title[可选]{标题}` 的纯文本。
fn extract_title(source: &str) -> Option<String> {
    let i = source.find("\\title")?;
    let mut rest = source[i + "\\title".len()..].trim_start();
    if rest.starts_with('[') {
        rest = &rest[rest.find(']')? + 1..];
        rest = rest.trim_start();
    }
    let rest = rest.strip_prefix('{')?;
    let mut depth = 0usize;
    for (i, c) in rest.char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => {
                let t = ym_latex::text::plain(&rest[..i])
                    .replace("---", "—")
                    .replace("--", "–");
                return (!t.is_empty()).then_some(t);
            }
            '}' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn lua_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn lua_filter(
    images: &BTreeMap<String, PathBuf>,
    labels: &BTreeMap<String, String>,
    chinese: bool,
) -> String {
    let mut s = String::from(
        "-- 由 YanMo 生成：替换图片、交叉引用编号，给图表标题加编号\nlocal images = {\n",
    );
    for (k, v) in images {
        s.push_str(&format!(
            "  [{}] = {},\n",
            lua_str(k),
            lua_str(&v.to_string_lossy())
        ));
    }
    s.push_str("}\nlocal labels = {\n");
    for (k, v) in labels {
        s.push_str(&format!("  [{}] = {},\n", lua_str(k), lua_str(v)));
    }
    let (fig, tab) = if chinese {
        ("图 ", "表 ")
    } else {
        ("Fig. ", "Table ")
    };
    s.push_str(&format!(
        r#"}}
local FIG, TAB = {fig}, {tab}

function Image(el)
  local r = images[el.src]
  if r then el.src = r end
  return el
end

function Link(el)
  local ref = el.attributes["reference"]
  local n = ref and labels[ref]
  if n then
    if el.attributes["reference-type"] == "eqref" then n = "(" .. n .. ")" end
    el.content = {{ pandoc.Str(n) }}
  end
  return el
end

local function number_caption(caption, prefix, n)
  local head = {{ pandoc.Strong({{ pandoc.Str(prefix .. n .. ".") }}), pandoc.Space() }}
  local blocks = caption.long
  if #blocks > 0 and blocks[1].content then
    for i = #head, 1, -1 do table.insert(blocks[1].content, 1, head[i]) end
  else
    caption.long = {{ pandoc.Plain(head) }}
  end
  return caption
end

-- 图的 label 可能在 Figure 上，也可能在其中的 Image 上
local function figure_label(el)
  if el.identifier ~= "" then return el.identifier end
  local found = ""
  el:walk({{ Image = function(img) if found == "" then found = img.identifier end end }})
  return found
end

function Figure(el)
  local n = labels[figure_label(el)]
  if n then el.caption = number_caption(el.caption, FIG, n) end
  return el
end

function Table(el)
  local n = labels[el.identifier]
  if n then el.caption = number_caption(el.caption, TAB, n) end
  return el
end
"#,
        fig = lua_str(fig),
        tab = lua_str(tab)
    ));
    s
}

fn modified(p: &Path) -> Option<SystemTime> {
    p.metadata().ok()?.modified().ok()
}

/// 把 PDF/EPS 转为 PNG。优先 Ghostscript（矢量渲染，220 dpi），macOS 上退而用 sips。
fn rasterize(src: &Path, dst: &Path, tc: &Toolchain) -> bool {
    if let Some(dir) = dst.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(gs) = &tc.ghostscript {
        let eps = src
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("eps") || e.eq_ignore_ascii_case("ps"));
        let ok = Command::new(gs)
            .args([
                "-q",
                "-dSAFER",
                "-dBATCH",
                "-dNOPAUSE",
                "-sDEVICE=png16m",
                "-r220",
            ])
            .args([
                "-dTextAlphaBits=4",
                "-dGraphicsAlphaBits=4",
                "-dFirstPage=1",
                "-dLastPage=1",
            ])
            .arg(if eps { "-dEPSCrop" } else { "-dUseCropBox" })
            .arg(format!("-sOutputFile={}", dst.display()))
            .arg(src)
            .status()
            .is_ok_and(|s| s.success());
        if ok && dst.is_file() {
            return true;
        }
    }
    if cfg!(target_os = "macos") {
        let ok = Command::new("/usr/bin/sips")
            .args(["-s", "format", "png"])
            .arg(src)
            .arg("--out")
            .arg(dst)
            .output()
            .is_ok_and(|o| o.status.success());
        return ok && dst.is_file();
    }
    false
}

/// 在项目根目录与 `.yanmo/` 中找第一个指定扩展名的文件。
fn find_asset(root: &Path, file_name: Option<&str>, ext: &str) -> Option<PathBuf> {
    for dir in [root.join(".yanmo"), root.to_path_buf()] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| match file_name {
                Some(n) => p.file_name().is_some_and(|f| f == n),
                None => p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)),
            })
            .collect();
        files.sort();
        if let Some(f) = files.into_iter().next() {
            return Some(f);
        }
    }
    None
}

pub fn export_docx(req: &ExportRequest, tc: &Toolchain) -> Result<ExportOutput, CompileError> {
    let pandoc = tc.pandoc.as_ref().ok_or(CompileError::NoPandoc)?;
    let started = Instant::now();
    let index = ym_latex::index_project(&req.root, &req.main)?;
    let source = std::fs::read_to_string(req.root.join(&req.main))?;
    let stem = req
        .main
        .file_stem()
        .expect("main is a file")
        .to_string_lossy()
        .into_owned();
    let main_dir_rel = req.main.parent().map(Path::to_path_buf).unwrap_or_default();
    let cwd = req.root.join(&main_dir_rel);
    let export_dir = req.root.join(EXPORT_DIR);
    std::fs::create_dir_all(&export_dir)?;
    let out_rel = req
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("{EXPORT_DIR}/{stem}.docx")));
    let out_abs = req.root.join(&out_rel);
    if let Some(dir) = out_abs.parent() {
        std::fs::create_dir_all(dir)?;
    }

    // 交叉引用编号
    let aux = req.root.join(BUILD_DIR).join(format!("{stem}.aux"));
    let labels = std::fs::read_to_string(&aux)
        .map(|a| read_aux_labels(&a))
        .unwrap_or_default();

    // 图片
    let mut images = BTreeMap::new();
    let mut converted = 0;
    let mut missing = Vec::new();
    let mut seen = HashSet::new();
    for g in index.graphics() {
        if !seen.insert(g.name.clone()) {
            continue;
        }
        let Some(rel) = index.resolve_graphic(&g.name) else {
            missing.push(g.name.clone());
            continue;
        };
        let abs = req.root.join(&rel);
        let ext = rel
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !matches!(ext.as_str(), "pdf" | "eps" | "ps") {
            images.insert(g.name.clone(), abs);
            continue;
        }
        // 同名位图（作者自己准备的 PNG/JPG）优先
        let sibling = ["png", "PNG", "jpg", "jpeg", "JPG"]
            .iter()
            .map(|e| abs.with_extension(e))
            .find(|p| p.is_file());
        if let Some(s) = sibling {
            images.insert(g.name.clone(), s);
            continue;
        }
        let dst = export_dir.join("media").join(rel.with_extension("png"));
        let fresh = matches!((modified(&dst), modified(&abs)), (Some(d), Some(s)) if d >= s);
        if fresh || rasterize(&abs, &dst, tc) {
            converted += usize::from(!fresh);
            images.insert(g.name.clone(), dst);
        } else {
            missing.push(g.name.clone());
        }
    }

    let title = extract_title(&source);
    let has_cjk = |t: &str| t.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let chinese = source.contains("ctex")
        || source.contains("xeCJK")
        || title.as_deref().is_some_and(has_cjk);
    let filter = export_dir.join("yanmo.lua");
    std::fs::write(&filter, lua_filter(&images, &labels, chinese))?;

    let mut cmd = Command::new(pandoc);
    cmd.current_dir(&cwd)
        .args([
            "-f",
            "latex",
            "-t",
            "docx",
            "--number-sections",
            "--citeproc",
        ])
        .arg("-o")
        .arg(&out_abs)
        .arg("--lua-filter")
        .arg(&filter);
    let mut resource = vec![cwd.clone()];
    resource.extend(index.graphics_paths().map(|d| cwd.join(d)));
    cmd.arg(format!(
        "--resource-path={}",
        std::env::join_paths(&resource)
            .unwrap_or_default()
            .to_string_lossy()
    ));
    for b in index.bib_files() {
        let name = if b.name.ends_with(".bib") {
            b.name.clone()
        } else {
            format!("{}.bib", b.name)
        };
        if cwd.join(&name).is_file() {
            cmd.arg(format!("--bibliography={name}"));
        }
    }
    let heading = if chinese {
        "参考文献"
    } else {
        "References"
    };
    cmd.args([
        "-M",
        &format!("reference-section-title={heading}"),
        "-M",
        "link-citations=true",
    ]);
    let csl = find_asset(&req.root, None, "csl");
    if let Some(c) = &csl {
        cmd.arg(format!("--csl={}", c.display()));
    }
    let reference_doc = find_asset(&req.root, Some("reference.docx"), "docx");
    if let Some(r) = &reference_doc {
        cmd.arg(format!("--reference-doc={}", r.display()));
    }

    // 预处理后的源码经标准输入传入；\\input 等相对路径仍以主文件所在目录为基准
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("piped");
        stdin.write_all(prepare_for_pandoc(&source).as_bytes())?;
    }
    let output = child.wait_with_output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(CompileError::Pandoc(stderr.trim().to_owned()));
    }
    let mut warnings: Vec<String> = Vec::new();
    for l in stderr.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if !warnings.iter().any(|w| w == l) {
            warnings.push(l.to_owned());
        }
    }
    let rel = |p: &Path| {
        p.strip_prefix(&req.root)
            .unwrap_or(p)
            .to_string_lossy()
            .replace('\\', "/")
    };
    Ok(ExportOutput {
        path: rel(&out_abs),
        duration_ms: started.elapsed().as_millis() as u64,
        numbered_refs: !labels.is_empty(),
        labels: labels.len(),
        figures_converted: converted,
        figures_missing: missing,
        csl: csl.as_deref().map(rel),
        reference_doc: reference_doc.as_deref().map(rel),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_aux_labels() {
        let aux = r"\relax
\newlabel{sec:intro}{{1}{1}{Introduction}{section.1}{}}
\newlabel{fig:study-area}{{1}{3}{Study area}{figure.caption.2}{}}
\newlabel{eq:cfi}{{{2}}{5}{}{equation.2.2}{}}
\newlabel{tab:panels@cref}{{[table][1][]1}{[1][4][]4}}
\newlabel{tab:panels}{{1}{4}{Panels}{table.caption.3}{}}
\newlabel{sec:a1}{{A.1}{20}{}{subsection.A.1}{}}";
        let l = read_aux_labels(aux);
        assert_eq!(l["sec:intro"], "1");
        assert_eq!(l["fig:study-area"], "1");
        assert_eq!(l["eq:cfi"], "2");
        assert_eq!(l["tab:panels"], "1");
        assert_eq!(l["sec:a1"], "A.1");
        assert!(!l.contains_key("tab:panels@cref"));
    }

    #[test]
    fn strips_low_level_blocks_and_title_options() {
        let src = "\\documentclass{cas-sc}\n\\makeatletter\n\\def\\x@y{1}\n\\makeatother\n\\ExplSyntaxOn\n\\group_end:\n\\ExplSyntaxOff\n\\begin{document}\n\\title[mode=title]{A--B}\n";
        let out = prepare_for_pandoc(src);
        assert!(!out.contains("x@y") && !out.contains("group_end"));
        assert!(out.contains("\\title{A--B}"));
        assert!(out.contains("\\begin{document}"));
    }

    #[test]
    fn extracts_title_with_options() {
        let src = r"\documentclass{cas-sc}\title[mode=title]{An H3--XGBoost \emph{framework}}";
        assert_eq!(
            extract_title(src).as_deref(),
            Some("An H3–XGBoost framework")
        );
        assert_eq!(extract_title(r"\title{Plain}").as_deref(), Some("Plain"));
    }
}
