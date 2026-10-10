//! 内置命令。新增功能 = 在这里注册一个命令。

use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{CommandError, CommandMeta, CommandResult, Registry, Workspace};

pub(crate) fn register(r: &mut Registry) {
    let path_param =
        json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]});
    let none = json!({"type": "object", "properties": {}});

    r.register(
        meta("project.open", "打开文件夹", path_param.clone(), false),
        project_open,
    );
    r.register(
        meta("project.info", "项目信息", none.clone(), false),
        project_info,
    );
    r.register(
        meta("project.tree", "项目文件树", none.clone(), false),
        project_tree,
    );
    r.register(
        meta("project.index", "大纲、图表与字数", none.clone(), false),
        project_index,
    );
    r.register(
        meta("project.set_main", "设为主文件", path_param.clone(), false),
        project_set_main,
    );
    r.register(
        meta(
            "export.docx",
            "导出为 Word（Pandoc）",
            json!({"type": "object", "properties": {"output": {"type": "string", "description": "相对项目根目录，默认 .yanmo/export/<主文件名>.docx"}}}),
            false,
        ),
        export_docx,
    );
    r.register(
        meta(
            "file.base64",
            "读取二进制文件（图片、PDF）",
            path_param.clone(),
            false,
        ),
        file_base64,
    );
    let id_param = json!({"type": "object", "properties": {"id": {"type": "string", "description": "DOI、arXiv 编号或链接"}}, "required": ["id"]});
    r.register(
        meta(
            "bib.fetch",
            "联网获取文献元数据（DOI / arXiv）",
            id_param.clone(),
            false,
        ),
        bib_fetch,
    );
    r.register(
        meta(
            "bib.add",
            "把 BibTeX 条目加入项目文献库",
            json!({"type": "object", "properties": {"bibtex": {"type": "string"}, "file": {"type": "string"}}, "required": ["bibtex"]}),
            true,
        ),
        bib_add,
    );
    r.register(
        meta("bib.import", "通过 DOI / arXiv 导入文献", id_param, true),
        bib_import,
    );
    r.register(
        meta(
            "asset.import",
            "导入图片到项目图片目录",
            json!({"type": "object", "properties": {
                "source": {"type": "string", "description": "本机图片的绝对路径（拖入时）"},
                "base64": {"type": "string", "description": "图片数据（粘贴时）"},
                "name": {"type": "string", "description": "文件名，粘贴时使用"}
            }}),
            true,
        ),
        asset_import,
    );
    r.register(
        meta("check.run", "检查项目问题", none.clone(), false),
        check_run,
    );
    r.register(
        meta(
            "compile.run",
            "编译论文",
            json!({"type": "object", "properties": {"engine": {"enum": ["tectonic", "xelatex", "pdflatex", "lualatex"]}}}),
            false,
        ),
        compile_run,
    );
    r.register(
        meta("compile.pdf", "读取编译出的 PDF", none.clone(), false),
        compile_pdf,
    );
    r.register(
        meta("compile.toolchain", "可用的编译引擎", none, false),
        compile_toolchain,
    );
    r.register(
        meta(
            "synctex.forward",
            "源码位置 → PDF 位置",
            json!({"type": "object", "properties": {"path": {"type": "string"}, "line": {"type": "integer"}}, "required": ["path", "line"]}),
            false,
        ),
        synctex_forward,
    );
    r.register(
        meta(
            "synctex.inverse",
            "PDF 位置 → 源码位置",
            json!({"type": "object", "properties": {"page": {"type": "integer"}, "x": {"type": "number"}, "y": {"type": "number"}}, "required": ["page", "x", "y"]}),
            false,
        ),
        synctex_inverse,
    );
    r.register(meta("file.read", "读取文件", path_param, false), file_read);
    r.register(
        meta(
            "file.write",
            "写入文件",
            json!({"type": "object", "properties": {"path": {"type": "string"}, "text": {"type": "string"}}, "required": ["path", "text"]}),
            true,
        ),
        file_write,
    );
}

fn meta(
    id: &'static str,
    title: &'static str,
    params_schema: Value,
    mutating: bool,
) -> CommandMeta {
    CommandMeta {
        id,
        title,
        params_schema,
        mutating,
    }
}

fn params<T: for<'de> Deserialize<'de>>(v: Value) -> Result<T, CommandError> {
    serde_json::from_value(v).map_err(|e| CommandError::InvalidParams(e.to_string()))
}

fn to_value<T: serde::Serialize>(v: &T) -> CommandResult {
    serde_json::to_value(v).map_err(|e| CommandError::Failed(e.to_string()))
}

#[derive(Deserialize)]
struct PathParam {
    path: String,
}

#[derive(Deserialize)]
struct WriteParam {
    path: String,
    text: String,
}

fn project_open(ws: &mut Workspace, p: Value) -> CommandResult {
    let PathParam { path } = params(p)?;
    ws.open(Path::new(&path))?;
    project_info(ws, Value::Null)
}

fn project_info(ws: &mut Workspace, _: Value) -> CommandResult {
    let root = ws.root()?;
    Ok(json!({
        "root": root,
        "name": root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        "main": ws.main(),
        "roots": ym_latex::find_roots(root),
    }))
}

#[derive(serde::Serialize)]
struct TreeNode {
    name: String,
    /// 相对项目根目录
    path: PathBuf,
    kind: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<TreeNode>,
}

const SKIP_DIRS: &[&str] = &["target", "node_modules", "_minted"];
const SKIP_EXTS: &[&str] = &[
    "aux",
    "log",
    "out",
    "toc",
    "lof",
    "lot",
    "bbl",
    "blg",
    "fls",
    "fdb_latexmk",
    "synctex",
    "gz",
    "xdv",
];

fn file_kind(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("tex" | "sty" | "cls") => "tex",
        Some("bib") => "bib",
        Some("pdf" | "png" | "jpg" | "jpeg" | "eps" | "svg") => "image",
        _ => "other",
    }
}

fn walk(root: &Path, rel: &Path, depth: u32) -> Vec<TreeNode> {
    let Ok(entries) = fs::read_dir(root.join(rel)) else {
        return Vec::new();
    };
    let mut nodes: Vec<TreeNode> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                return None;
            }
            let path = rel.join(&name);
            if e.path().is_dir() {
                let children = if depth > 0 {
                    walk(root, &path, depth - 1)
                } else {
                    Vec::new()
                };
                Some(TreeNode {
                    name,
                    path,
                    kind: "dir",
                    children,
                })
            } else {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if SKIP_EXTS.contains(&ext) {
                    return None;
                }
                let kind = file_kind(&path);
                Some(TreeNode {
                    name,
                    path,
                    kind,
                    children: Vec::new(),
                })
            }
        })
        .collect();
    nodes.sort_by(|a, b| (a.kind != "dir", &a.name).cmp(&(b.kind != "dir", &b.name)));
    nodes
}

fn project_set_main(ws: &mut Workspace, p: Value) -> CommandResult {
    let PathParam { path } = params(p)?;
    ws.set_main(Path::new(&path), true)?;
    project_info(ws, Value::Null)
}

fn project_tree(ws: &mut Workspace, _: Value) -> CommandResult {
    let root = ws.root()?.to_path_buf();
    to_value(&walk(&root, Path::new(""), 6))
}

fn project_index(ws: &mut Workspace, _: Value) -> CommandResult {
    let index = ws.index()?;
    // 附上 \includegraphics 解析后的项目内路径，供缩略图使用
    let assets: Vec<Value> = index
        .assets()
        .map(|a| {
            let mut v = serde_json::to_value(a).unwrap_or(Value::Null);
            let paths: Vec<Option<PathBuf>> = a
                .files
                .iter()
                .map(|f| index.resolve_graphic(&f.to_string_lossy()))
                .collect();
            if let Value::Object(m) = &mut v {
                m.insert("paths".into(), json!(paths));
            }
            v
        })
        .collect();
    Ok(json!({
        "main": index.main,
        "total_words": index.total_words(),
        "front_words": index.front_words,
        "outline": index.outline,
        "assets": assets,
        "labels": index.labels().map(|l| &l.name).collect::<Vec<_>>(),
        "files": index.files.iter().map(|f| &f.path).collect::<Vec<_>>(),
    }))
}

fn check_run(ws: &mut Workspace, _: Value) -> CommandResult {
    let index = ws.index()?;
    let report = ym_check::run(index);
    let issues: Vec<Value> = report
        .issues
        .iter()
        .map(|issue| {
            let (file, line) = issue.location();
            let mut v = serde_json::to_value(issue).unwrap_or(Value::Null);
            if let Value::Object(map) = &mut v {
                map.insert("severity".into(), json!(issue.severity()));
                map.insert("message".into(), json!(issue.message()));
                map.insert("file".into(), json!(file));
                map.insert("line".into(), json!(line));
            }
            v
        })
        .collect();
    // 标出真正未被任何文档引用的条目（与问题列表一致，考虑了补充材料等兄弟文档）
    let unused: std::collections::HashSet<&str> = report
        .issues
        .iter()
        .filter_map(|i| match i {
            ym_check::Issue::UnusedBibEntry { key, .. } => Some(key.as_str()),
            _ => None,
        })
        .collect();
    let bib: Vec<Value> = report
        .bib
        .iter()
        .map(|e| {
            let mut v = serde_json::to_value(e).unwrap_or(Value::Null);
            if let Value::Object(m) = &mut v {
                m.insert("unused".into(), json!(unused.contains(e.key.as_str())));
            }
            v
        })
        .collect();
    Ok(json!({
        "errors": report.count(ym_check::Severity::Error),
        "warnings": report.count(ym_check::Severity::Warning),
        "issues": issues,
        "bib": bib,
    }))
}

fn file_read(ws: &mut Workspace, p: Value) -> CommandResult {
    let PathParam { path } = params(p)?;
    let text = fs::read_to_string(ws.resolve(&path)?)?;
    Ok(json!({ "path": path, "text": text }))
}

/// 读取图片等二进制文件，供缩略图与预览使用；超过 50MB 的文件拒绝读取。
fn file_base64(ws: &mut Workspace, p: Value) -> CommandResult {
    use base64::Engine as _;
    let PathParam { path } = params(p)?;
    let abs = ws.resolve(&path)?;
    let size = fs::metadata(&abs)?.len();
    if size > 50 * 1024 * 1024 {
        return Err(CommandError::Failed(format!(
            "文件过大（{} MB）",
            size / 1024 / 1024
        )));
    }
    let mime = match abs
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    };
    let bytes = fs::read(&abs)?;
    Ok(
        json!({ "path": path, "mime": mime, "base64": base64::engine::general_purpose::STANDARD.encode(bytes) }),
    )
}

#[derive(Deserialize)]
struct IdParam {
    id: String,
}

fn bib_fetch(_: &mut Workspace, p: Value) -> CommandResult {
    let IdParam { id } = params(p)?;
    let ident = ym_bib::import::detect(&id).ok_or_else(|| {
        CommandError::InvalidParams(format!("无法识别为 DOI 或 arXiv 编号：{id}"))
    })?;
    let entry = ym_bib::import::fetch(&ident).map_err(|e| CommandError::Failed(e.to_string()))?;
    Ok(json!({
        "key": entry.key,
        "title": entry.field("title"),
        "bibtex": ym_bib::import::to_bibtex(&entry),
    }))
}

#[derive(Deserialize)]
struct AddParam {
    bibtex: String,
    file: Option<String>,
}

fn bib_add(ws: &mut Workspace, p: Value) -> CommandResult {
    let AddParam { bibtex, file } = params(p)?;
    let parsed = ym_bib::parse(Path::new("new.bib"), &bibtex);
    let mut entry = parsed
        .entries
        .into_iter()
        .next()
        .ok_or_else(|| CommandError::InvalidParams("没有可识别的 BibTeX 条目".into()))?;

    let index = ws.index()?;
    let (existing, _) = ym_check::load_bib(index);
    let base = index.base_dir();
    let bib_files: Vec<String> = index
        .bib_files()
        .map(|b| {
            if b.name.ends_with(".bib") {
                b.name.clone()
            } else {
                format!("{}.bib", b.name)
            }
        })
        .collect();

    // 查重：同一 DOI、同一 arXiv 编号，或标题相同（忽略大小写、标点与 BibTeX 花括号）视为同一篇
    let norm = |v: Option<&str>| v.map(|s| s.trim().to_ascii_lowercase());
    let title_key = |v: Option<&str>| {
        v.map(|t| {
            t.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|t| t.len() >= 12)
    };
    let (doi, eprint, title) = (
        norm(entry.field("doi")),
        norm(entry.field("eprint")),
        title_key(entry.field("title")),
    );
    let same = existing.iter().find(|e| {
        (doi.is_some() && norm(e.field("doi")) == doi)
            || (eprint.is_some() && norm(e.field("eprint")) == eprint)
            || (title.is_some() && title_key(e.field("title")) == title)
    });
    if let Some(e) = same {
        return Ok(json!({ "key": e.key, "duplicate": true, "file": e.span.file }));
    }
    // key 冲突时加后缀 b、c……
    let taken = |k: &str| existing.iter().any(|e| e.key == k);
    if taken(&entry.key) {
        let stem = entry.key.clone();
        entry.key = (b'b'..=b'z')
            .map(|c| format!("{stem}{}", c as char))
            .find(|k| !taken(k))
            .unwrap_or_else(|| format!("{stem}x"));
    }

    let target = match file.or_else(|| bib_files.first().cloned()) {
        Some(f) => base.join(f),
        None => base.join("references.bib"),
    };
    let rel = target.to_string_lossy().replace('\\', "/");
    let abs = ws.resolve(&rel)?;
    let created = !abs.is_file();
    let mut text = fs::read_to_string(&abs).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(&ym_bib::import::to_bibtex(&entry));
    fs::write(&abs, text)?;
    ws.invalidate();
    Ok(json!({
        "key": entry.key,
        "duplicate": false,
        "file": rel,
        "created": created,
        // 新建的 .bib 还需要在正文里用 \bibliography 引入
        "needs_bibliography": bib_files.is_empty(),
    }))
}

fn bib_import(ws: &mut Workspace, p: Value) -> CommandResult {
    let fetched = bib_fetch(ws, p)?;
    bib_add(ws, json!({ "bibtex": fetched["bibtex"] }))
}

#[derive(Deserialize)]
struct AssetParam {
    source: Option<String>,
    base64: Option<String>,
    name: Option<String>,
}

/// 文件名转成适合 LaTeX 的形式：小写、空格与特殊字符换成连字符。
fn slug(stem: &str) -> String {
    let mut out = String::new();
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "figure".into()
    } else {
        out
    }
}

fn asset_import(ws: &mut Workspace, p: Value) -> CommandResult {
    use base64::Engine as _;
    const EXTS: &[&str] = &["png", "jpg", "jpeg", "pdf", "eps"];
    let AssetParam {
        source,
        base64,
        name,
    } = params(p)?;
    let (bytes, file_name) = match (source, base64) {
        (Some(src), _) => {
            let path = PathBuf::from(&src);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (fs::read(&path)?, name)
        }
        (None, Some(b64)) => (
            base64::engine::general_purpose::STANDARD
                .decode(b64.as_bytes())
                .map_err(|e| CommandError::InvalidParams(e.to_string()))?,
            name.unwrap_or_else(|| "pasted.png".into()),
        ),
        _ => return Err(CommandError::InvalidParams("需要 source 或 base64".into())),
    };
    let fname = Path::new(&file_name);
    let ext = fname
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !EXTS.contains(&ext.as_str()) {
        return Err(CommandError::InvalidParams(format!(
            "LaTeX 不能直接插入 .{ext} 文件，支持 PNG、JPG、PDF、EPS"
        )));
    }
    let stem = slug(
        &fname
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    );

    // 目标目录：\graphicspath > 已有的常见图片目录 > 新建 figures/
    let root = ws.root()?.to_path_buf();
    let index = ws.index()?;
    let base = index.base_dir();
    let graphics_path = index
        .graphics_paths()
        .next()
        .map(|d| d.trim_end_matches('/').to_owned());
    let dir_rel = graphics_path.clone().unwrap_or_else(|| {
        ["figures", "Figures", "figs", "fig", "images", "img"]
            .iter()
            .find(|d| root.join(&base).join(d).is_dir())
            .map(|d| d.to_string())
            .unwrap_or_else(|| "figures".into())
    });
    let dir = root.join(&base).join(&dir_rel);
    fs::create_dir_all(&dir)?;
    let mut final_name = format!("{stem}.{ext}");
    let mut n = 2;
    while dir.join(&final_name).exists() {
        final_name = format!("{stem}-{n}.{ext}");
        n += 1;
    }
    fs::write(dir.join(&final_name), bytes)?;
    ws.invalidate();

    let rel = base
        .join(&dir_rel)
        .join(&final_name)
        .to_string_lossy()
        .replace('\\', "/");
    // 目录已在 \graphicspath 中时只写文件名；pdfLaTeX/XeLaTeX 会自动补全扩展名
    let include_dir = if graphics_path.is_some() {
        String::new()
    } else {
        format!("{dir_rel}/")
    };
    let include_stem = Path::new(&final_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(json!({
        "path": rel,
        "include": format!("{include_dir}{include_stem}"),
        "label": format!("fig:{}", slug(&include_stem)),
    }))
}

fn file_write(ws: &mut Workspace, p: Value) -> CommandResult {
    let WriteParam { path, text } = params(p)?;
    let abs = ws.resolve(&path)?;
    if let Some(dir) = abs.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&abs, text)?;
    ws.invalidate();
    Ok(json!({ "path": path }))
}

#[derive(Deserialize, Default)]
struct CompileParam {
    engine: Option<ym_model::Engine>,
}

fn compile_run(ws: &mut Workspace, p: Value) -> CommandResult {
    let CompileParam { engine } = if p.is_null() {
        CompileParam::default()
    } else {
        params(p)?
    };
    let job = ws.compile_job(engine)?;
    let out = job.run()?;
    ws.finish_compile(out.clone());
    to_value(&out)
}

#[derive(Deserialize, Default)]
struct ExportParam {
    output: Option<PathBuf>,
}

fn export_docx(ws: &mut Workspace, p: Value) -> CommandResult {
    let ExportParam { output } = if p.is_null() {
        ExportParam::default()
    } else {
        params(p)?
    };
    let out = ws.export_job(output)?.run()?;
    to_value(&out)
}

fn compile_pdf(ws: &mut Workspace, _: Value) -> CommandResult {
    use base64::Engine as _;
    let rel = ws
        .pdf_path()
        .ok_or_else(|| CommandError::Failed("尚未编译出 PDF".into()))?;
    let bytes = fs::read(ws.root()?.join(&rel))?;
    Ok(json!({ "path": rel, "base64": base64::engine::general_purpose::STANDARD.encode(bytes) }))
}

fn compile_toolchain(ws: &mut Workspace, _: Value) -> CommandResult {
    to_value(ws.toolchain())
}

#[derive(Deserialize)]
struct ForwardParam {
    path: String,
    line: u32,
}

#[derive(Deserialize)]
struct InverseParam {
    page: u32,
    x: f64,
    y: f64,
}

fn synctex_forward(ws: &mut Workspace, p: Value) -> CommandResult {
    let ForwardParam { path, line } = params(p)?;
    to_value(&ws.synctex()?.forward(&path, line))
}

fn synctex_inverse(ws: &mut Workspace, p: Value) -> CommandResult {
    let InverseParam { page, x, y } = params(p)?;
    to_value(&ws.synctex()?.inverse(page, x, y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> (Registry, Workspace) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample-paper");
        let r = Registry::with_builtins();
        let mut ws = Workspace::default();
        r.execute(&mut ws, "project.open", json!({"path": root}))
            .unwrap();
        (r, ws)
    }

    #[test]
    fn open_index_check() {
        let (r, mut ws) = sample();
        let info = r.execute(&mut ws, "project.info", Value::Null).unwrap();
        assert_eq!(info["main"], "main.tex");
        let index = r.execute(&mut ws, "project.index", Value::Null).unwrap();
        assert_eq!(index["outline"][0]["title"], "引言");
        let check = r.execute(&mut ws, "check.run", Value::Null).unwrap();
        assert_eq!(check["errors"], 4);
        assert_eq!(check["issues"][0]["severity"], "error");
        let tree = r.execute(&mut ws, "project.tree", Value::Null).unwrap();
        assert!(tree
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["name"] == "sections"));
    }

    #[test]
    fn rejects_paths_outside_project() {
        let (r, mut ws) = sample();
        for bad in ["../secret.txt", "/etc/passwd", ""] {
            let err = r
                .execute(&mut ws, "file.read", json!({"path": bad}))
                .unwrap_err();
            assert!(matches!(err, CommandError::InvalidParams(_)), "{bad}");
        }
        let ok = r
            .execute(&mut ws, "file.read", json!({"path": "sections/intro.tex"}))
            .unwrap();
        assert!(ok["text"].as_str().unwrap().contains("引言"));
    }
}
