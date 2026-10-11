//! 内置命令。新增功能 = 在这里注册一个命令。

use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{CommandError, CommandMeta, CommandResult, Registry, Workspace};
use ym_model::tr;

pub(crate) fn register(r: &mut Registry) {
    let path_param =
        json!({"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]});
    let none = json!({"type": "object", "properties": {}});

    r.register(
        meta(
            "app.set_lang",
            ("切换界面语言", "Set Interface Language"),
            json!({"type": "object", "properties": {"lang": {"enum": ["zh", "en"]}}, "required": ["lang"]}),
            false,
        ),
        app_set_lang,
    );
    r.register(
        meta(
            "project.open",
            ("打开文件夹", "Open Folder"),
            path_param.clone(),
            false,
        ),
        project_open,
    );
    r.register(
        meta(
            "project.info",
            ("项目信息", "Project Info"),
            none.clone(),
            false,
        ),
        project_info,
    );
    r.register(
        meta(
            "project.tree",
            ("项目文件树", "Project File Tree"),
            none.clone(),
            false,
        ),
        project_tree,
    );
    r.register(
        meta(
            "project.index",
            ("大纲、图表与字数", "Outline, Figures and Word Count"),
            none.clone(),
            false,
        ),
        project_index,
    );
    r.register(
        meta(
            "project.set_main",
            ("设为主文件", "Set as Main File"),
            path_param.clone(),
            false,
        ),
        project_set_main,
    );
    r.register(
        meta(
            "export.docx",
            ("导出为 Word（Pandoc）", "Export to Word (Pandoc)"),
            json!({"type": "object", "properties": {"output": {"type": "string", "description": "Relative to the project root; defaults to .yanmo/export/<main>.docx"}}}),
            false,
        ),
        export_docx,
    );
    r.register(
        meta(
            "file.base64",
            (
                "读取二进制文件（图片、PDF）",
                "Read Binary File (Image, PDF)",
            ),
            path_param.clone(),
            false,
        ),
        file_base64,
    );
    let id_param = json!({"type": "object", "properties": {"id": {"type": "string", "description": "A DOI, an arXiv ID, or a link to either"}}, "required": ["id"]});
    r.register(
        meta(
            "bib.fetch",
            (
                "联网获取文献元数据（DOI / arXiv）",
                "Fetch Reference Metadata (DOI / arXiv)",
            ),
            id_param.clone(),
            false,
        ),
        bib_fetch,
    );
    r.register(
        meta(
            "bib.add",
            ("把 BibTeX 条目加入项目文献库", "Add BibTeX Entry to Bibliography"),
            json!({"type": "object", "properties": {"bibtex": {"type": "string"}, "file": {"type": "string"}}, "required": ["bibtex"]}),
            true,
        ),
        bib_add,
    );
    r.register(
        meta(
            "bib.add_field",
            ("补充文献字段", "Add Field to Reference"),
            json!({"type": "object", "properties": {
                "key": {"type": "string"},
                "field": {"type": "string", "description": "e.g. doi"},
                "value": {"type": "string"}
            }, "required": ["key", "field", "value"]}),
            true,
        ),
        bib_add_field,
    );
    r.register(
        meta(
            "zotero.search",
            ("搜索 Zotero", "Search Zotero"),
            json!({"type": "object", "properties": {
                "query": {"type": "string", "description": "Words from the title, an author or a year"},
                "limit": {"type": "integer"}
            }, "required": ["query"]}),
            false,
        ),
        zotero_search,
    );
    r.register(
        meta(
            "zotero.add",
            ("从 Zotero 加入文献库", "Add from Zotero"),
            json!({"type": "object", "properties": {
                "keys": {"type": "array", "items": {"type": "string"}, "description": "Zotero item keys from zotero_search"}
            }, "required": ["keys"]}),
            true,
        ),
        zotero_add,
    );
    r.register(
        meta(
            "bib.abstracts",
            ("获取文献摘要", "Fetch Reference Abstracts"),
            json!({"type": "object", "properties": {
                "keys": {"type": "array", "items": {"type": "string"}}
            }, "required": ["keys"]}),
            false,
        ),
        bib_abstracts,
    );
    r.register(
        meta(
            "bib.verify",
            ("核查文献", "Verify References"),
            json!({"type": "object", "properties": {
                "keys": {"type": "array", "items": {"type": "string"}, "description": "Citation keys to check; all entries when omitted"}
            }}),
            false,
        ),
        bib_verify,
    );
    r.register(
        meta(
            "bib.import",
            (
                "通过 DOI / arXiv 导入文献",
                "Import Reference by DOI / arXiv",
            ),
            id_param,
            true,
        ),
        bib_import,
    );
    r.register(
        meta(
            "asset.import",
            ("导入图片到项目图片目录", "Import Image into Project"),
            json!({"type": "object", "properties": {
                "source": {"type": "string", "description": "Absolute path of a local image (when dropped)"},
                "base64": {"type": "string", "description": "Image data (when pasted)"},
                "name": {"type": "string", "description": "File name, used when pasting"}
            }}),
            true,
        ),
        asset_import,
    );
    r.register(
        meta(
            "check.run",
            ("检查项目问题", "Check Project"),
            none.clone(),
            false,
        ),
        check_run,
    );
    r.register(
        meta(
            "compile.run",
            ("编译论文", "Compile Paper"),
            json!({"type": "object", "properties": {"engine": {"enum": ["tectonic", "xelatex", "pdflatex", "lualatex"]}, "timeout_secs": {"type": "integer"}}}),
            false,
        ),
        compile_run,
    );
    r.register(
        meta(
            "compile.pdf",
            ("读取编译出的 PDF", "Read Compiled PDF"),
            none.clone(),
            false,
        ),
        compile_pdf,
    );
    r.register(
        meta(
            "compile.toolchain",
            ("可用的编译引擎", "Available TeX Engines"),
            json!({"type": "object", "properties": {"refresh": {"type": "boolean", "description": "Detect again (after installing a TeX distribution)"}}}),
            false,
        ),
        compile_toolchain,
    );
    r.register(
        meta(
            "compile.cancel",
            ("停止编译", "Stop Compiling"),
            none.clone(),
            false,
        ),
        compile_cancel,
    );
    r.register(
        meta(
            "compile.clean",
            ("清理编译缓存", "Clean Build Files"),
            none.clone(),
            false,
        ),
        compile_clean,
    );
    r.register(
        meta(
            "project.export_zip",
            ("导出为 Overleaf 项目（zip）", "Export as Overleaf Project (zip)"),
            json!({"type": "object", "properties": {
                "output": {"type": "string", "description": "Relative to the project root; defaults to .yanmo/export/<project>.zip"}
            }}),
            false,
        ),
        project_export_zip,
    );
    r.register(
        meta(
            "project.import_zip",
            ("导入 Overleaf 项目（zip）", "Import Overleaf Project (zip)"),
            json!({"type": "object", "properties": {
                "zip": {"type": "string", "description": "Absolute path of the zip file"},
                "parent": {"type": "string", "description": "Absolute path of the folder to create the project in"},
                "name": {"type": "string", "description": "Name of the new folder; defaults to the zip file name"}
            }, "required": ["zip", "parent"]}),
            true,
        ),
        project_import_zip,
    );
    r.register(
        meta(
            "project.templates",
            ("论文模板", "Paper Templates"),
            none.clone(),
            false,
        ),
        project_templates,
    );
    r.register(
        meta(
            "project.create",
            ("从模板新建论文", "New Paper from Template"),
            json!({"type": "object", "properties": {
                "template": {"type": "string", "description": "Template id from project_templates"},
                "parent": {"type": "string", "description": "Absolute path of an existing folder to create the project in"},
                "name": {"type": "string", "description": "Name of the new project folder"},
                "title": {"type": "string", "description": "Paper title"}
            }, "required": ["template", "parent", "name"]}),
            true,
        ),
        project_create,
    );
    r.register(
        meta(
            "project.settings",
            ("项目设置", "Project Settings"),
            none.clone(),
            false,
        ),
        project_settings,
    );
    r.register(
        meta(
            "project.update_settings",
            ("修改项目设置", "Change Project Settings"),
            json!({"type": "object", "properties": {
                "engine": {"enum": ["auto", "tectonic", "xelatex", "pdflatex", "lualatex"]},
                "shell_escape": {"type": "boolean"}
            }}),
            false,
        ),
        project_update_settings,
    );
    r.register(
        meta(
            "synctex.forward",
            ("源码位置 → PDF 位置", "Source → PDF Position"),
            json!({"type": "object", "properties": {"path": {"type": "string"}, "line": {"type": "integer"}}, "required": ["path", "line"]}),
            false,
        ),
        synctex_forward,
    );
    r.register(
        meta(
            "synctex.inverse",
            ("PDF 位置 → 源码位置", "PDF → Source Position"),
            json!({"type": "object", "properties": {"page": {"type": "integer"}, "x": {"type": "number"}, "y": {"type": "number"}}, "required": ["page", "x", "y"]}),
            false,
        ),
        synctex_inverse,
    );
    r.register(
        meta(
            "outline.move",
            ("移动章节", "Move Section"),
            json!({"type": "object", "properties": {
                "id": {"type": "string", "description": "Section id from project_index (its label, or a generated id)"},
                "target": {"type": "string", "description": "Id of a section at the same level"},
                "position": {"enum": ["before", "after"]}
            }, "required": ["id", "target", "position"]}),
            true,
        ),
        outline_move,
    );
    r.register(
        meta(
            "outline.extract",
            ("拆成独立文件", "Move Section to Its Own File"),
            json!({"type": "object", "properties": {
                "id": {"type": "string", "description": "Section id from project_index"}
            }, "required": ["id"]}),
            true,
        ),
        outline_extract,
    );
    r.register(
        meta(
            "outline.extract_undo",
            ("撤销拆分", "Undo Moving Section to File"),
            json!({"type": "object", "properties": {
                "parent": {"type": "string"}, "file": {"type": "string"},
                "parent_old": {"type": "string"}, "parent_new": {"type": "string"}, "text": {"type": "string"}
            }, "required": ["parent", "file", "parent_old", "parent_new", "text"]}),
            true,
        ),
        outline_extract_undo,
    );
    r.register(
        meta("file.read", ("读取文件", "Read File"), path_param, false),
        file_read,
    );
    r.register(
        meta(
            "file.write",
            ("写入文件", "Write File"),
            json!({"type": "object", "properties": {"path": {"type": "string"}, "text": {"type": "string"}}, "required": ["path", "text"]}),
            true,
        ),
        file_write,
    );
}

fn meta(
    id: &'static str,
    (title, title_en): (&'static str, &'static str),
    params_schema: Value,
    mutating: bool,
) -> CommandMeta {
    CommandMeta {
        id,
        title,
        title_en,
        description: describe(id),
        params_schema,
        mutating,
    }
}

/// 给 Agent 的命令说明。路径一律相对项目根目录。
fn describe(id: &str) -> &'static str {
    match id {
        "app.set_lang" => "Set the language (zh or en) of messages produced by later commands.",
        "project.open" => "Open a LaTeX project folder (absolute path) and detect its main file. Later commands work on this project.",
        "project.info" => "Project root, the main .tex file, and every standalone document (file with \\documentclass) in the folder.",
        "project.tree" => "The project's file tree (tex, bib, images and other files), excluding hidden folders and build output.",
        "project.index" => "Structure of the paper: section outline with labels, file locations and word counts; figures, tables and equations with captions, labels and reference locations; all labels; all included files. Re-parsed from source on every call.",
        "project.set_main" => "Choose which standalone .tex document is the main file for indexing, checking and compiling. Saved in .yanmo/project.toml.",
        "export.docx" => "Convert the paper to Word (.docx) with Pandoc. Cross-references and figure numbers match the PDF if the paper was compiled first. Writes into .yanmo/export/ unless an output path is given. Requires Pandoc.",
        "file.base64" => "Read a binary file (image or PDF) as base64.",
        "bib.fetch" => "Look up a DOI or arXiv ID online and return a BibTeX entry without changing any file.",
        "bib.add" => "Append a BibTeX entry to the project's .bib file (creates refs.bib if there is none). Returns the key; reports an existing duplicate instead of adding it twice.",
        "bib.import" => "Fetch a DOI or arXiv ID online and append the entry to the project's .bib file. Returns the citation key to use with \\cite.",
        "asset.import" => "Copy an image into the project's figures folder and return the path and a suggested label for \\includegraphics. Accepts PNG, JPG, PDF and EPS.",
        "check.run" => "Check the whole paper for problems: undefined \\ref and \\cite, duplicate labels and bibliography keys, missing image files, bib syntax errors, figures and references that are never cited. Returns issues with file and line, plus the parsed bibliography.",
        "compile.run" => "Compile the paper to PDF with the project's engine (Tectonic by default, or latexmk with xelatex / pdflatex / lualatex). Returns success, the PDF path, the log path, and errors and warnings with file and line. The first Tectonic build downloads packages and can take minutes. Optional timeout_secs stops a build that runs too long.",
        "compile.pdf" => "Read the last compiled PDF as base64.",
        "compile.toolchain" => "Which TeX engines and helper tools (Tectonic, latexmk, Pandoc, Ghostscript) are available.",
        "synctex.forward" => "Find where a source line appears in the compiled PDF (page and rectangle in PDF points).",
        "synctex.inverse" => "Find the source file and line for a point on a PDF page (PDF points, origin at the top left).",
        "outline.move" => "Move a section, with its subsections, before or after another section of the same level. Only one file is rewritten and the moved text is kept as is; when each section is its own file, the \\input lines are reordered instead. Fails if the move would need to split a file.",
        "outline.extract" => "Move a section (with its subsections) into a new .tex file and put an \\input line in its place. The file goes into the folder the paper already uses for included files (sections/ by default) and is named after the section label or title. The compiled paper is unchanged.",
        "outline.extract_undo" => "Undo outline.extract: restores the parent file and deletes the new file, only if neither has changed since.",
        "bib.verify" => "Look up each bibliography entry on Crossref (or arXiv) to confirm it exists and that its title, first author and year match the registered record; suggests DOIs for entries without one. Sends only bibliographic data (title, first author, year, DOI). Results: ok, mismatch, not_found, skipped, error.",
        "bib.add_field" => "Add a missing field (such as doi) to a bibliography entry. Inserts one line in the entry, keeping its indentation and alignment; fails if the field already exists.",
        "bib.abstracts" => "Fetch the abstracts of the given references from Semantic Scholar (by DOI, arXiv ID or title), falling back to Crossref. Useful for checking whether a citation supports a claim. Entries without an available abstract are returned with abstract = null.",
        "zotero.search" => "Search the user's local Zotero library by title, author or year (requires Zotero running with its local API enabled). Returns Zotero item keys with title, authors, year, type and DOI.",
        "zotero.add" => "Export the given Zotero items as BibTeX and append them to the project's .bib file, removing local file paths, personal notes and abstracts and generating citation keys. Reports duplicates instead of adding them twice. Returns the citation keys.",
        "compile.cancel" => "Stop the compilation that is currently running.",
        "compile.clean" => "Delete the build directory (.yanmo/build/) so the next compile starts from scratch.",
        "project.settings" => "Project build settings from .yanmo/project.toml: engine (auto, tectonic, xelatex, pdflatex, lualatex), shell_escape and the main file.",
        "project.update_settings" => "Change the project's build settings (engine, shell_escape); saved in .yanmo/project.toml.",
        "project.templates" => "List the paper templates (id, name, description) available for project_create.",
        "project.create" => "Create a new paper project from a template in a new folder <parent>/<name>, with main.tex, a sections/ folder, refs.bib and figures/. Returns the new folder; open it with project_open.",
        "project.export_zip" => "Package the project's source files as a zip that can be uploaded to Overleaf (New Project → Upload Project). Skips hidden folders such as .yanmo and .git, build files, Office documents, archives and PDFs compiled from a .tex of the same name (listed in omitted); too_large is true above Overleaf's 50 MB upload limit. Adds a latexmkrc so Overleaf uses XeLaTeX or LuaLaTeX when the paper needs it.",
        "project.import_zip" => "Unpack a project zip downloaded from Overleaf into a new folder <parent>/<name> and return it; open it with project_open.",
        "file.read" => "Read a text file in the project.",
        "file.write" => "Overwrite a text file in the project.",
        _ => "",
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
        return Err(CommandError::Failed(tr!(
            "文件过大（{mb} MB）",
            "File too large ({mb} MB)",
            mb = size / 1024 / 1024
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

#[derive(Deserialize)]
struct SectionParam {
    id: String,
}

/// 把章节拆成独立文件。返回撤销所需的内容。
fn outline_extract(ws: &mut Workspace, p: Value) -> CommandResult {
    let SectionParam { id } = params(p)?;
    ws.invalidate();
    let index = ws.index()?;
    let plan = ym_latex::reorder::plan_extract(index, &id)
        .map_err(|e| CommandError::Failed(e.to_string()))?;
    let file = ws.resolve(&plan.file.to_string_lossy())?;
    if file.exists() {
        return Err(CommandError::Failed(tr!(
            "文件已存在：{p}",
            "File already exists: {p}",
            p = plan.file.display()
        )));
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&file, &plan.text)?;
    fs::write(
        ws.resolve(&plan.parent.to_string_lossy())?,
        &plan.parent_new,
    )?;
    ws.invalidate();
    Ok(json!({
        "parent": plan.parent,
        "file": plan.file,
        "parent_old": plan.parent_old,
        "parent_new": plan.parent_new,
        "text": plan.text,
    }))
}

#[derive(Deserialize)]
struct ExtractUndo {
    parent: String,
    file: String,
    parent_old: String,
    parent_new: String,
    text: String,
}

/// 撤销拆分：两个文件都与拆分后一致时才恢复，避免覆盖之后的修改。
fn outline_extract_undo(ws: &mut Workspace, p: Value) -> CommandResult {
    let u: ExtractUndo = params(p)?;
    let parent = ws.resolve(&u.parent)?;
    let file = ws.resolve(&u.file)?;
    let unchanged = fs::read_to_string(&parent).ok().as_deref() == Some(u.parent_new.as_str())
        && fs::read_to_string(&file).ok().as_deref() == Some(u.text.as_str());
    if !unchanged {
        return Err(CommandError::Failed(tr!(
            "文件在拆分后已被修改，无法自动撤销",
            "The files changed after the split, so it cannot be undone automatically"
        )));
    }
    fs::write(&parent, &u.parent_old)?;
    fs::remove_file(&file)?;
    ws.invalidate();
    Ok(json!({ "parent": u.parent }))
}

#[derive(Deserialize, Default)]
struct KeysParam {
    #[serde(default)]
    keys: Vec<String>,
}

#[derive(Deserialize)]
struct FieldParam {
    key: String,
    field: String,
    value: String,
}

/// 给条目补一个缺少的字段，只插入一行。
fn bib_add_field(ws: &mut Workspace, p: Value) -> CommandResult {
    let FieldParam { key, field, value } = params(p)?;
    let field = field.trim().to_ascii_lowercase();
    let entry = ws
        .bib_entries(std::slice::from_ref(&key))?
        .into_iter()
        .next()
        .ok_or_else(|| {
            CommandError::Failed(tr!(
                "文献库中没有 {key}",
                "{key} is not in the bibliography"
            ))
        })?;
    if entry.field(&field).is_some() {
        return Err(CommandError::Failed(tr!(
            "{key} 已有 {field} 字段",
            "{key} already has a {field} field"
        )));
    }
    let path = ws.resolve(&entry.span.file.to_string_lossy())?;
    let text = fs::read_to_string(&path)?;
    let (start, end) = (entry.span.start, entry.span.end.min(text.len()));
    let updated = text
        .get(start..end)
        .and_then(|e| ym_bib::add_field(e, &field, value.trim()))
        .ok_or_else(|| {
            CommandError::Failed(tr!(
                "无法识别 {key} 的条目格式",
                "Cannot parse the entry for {key}"
            ))
        })?;
    fs::write(
        &path,
        format!("{}{updated}{}", &text[..start], &text[end..]),
    )?;
    ws.invalidate();
    Ok(json!({ "key": key, "file": entry.span.file }))
}

#[derive(Deserialize)]
struct ZoteroQuery {
    query: String,
    limit: Option<u32>,
}

fn zotero_search(_: &mut Workspace, p: Value) -> CommandResult {
    let ZoteroQuery { query, limit } = params(p)?;
    let items = ym_bib::zotero::search(query.trim(), limit.unwrap_or(20).min(50))
        .map_err(|e| CommandError::Failed(e.to_string()))?;
    Ok(json!({ "items": items }))
}

/// 从 Zotero 导出条目并逐条加入文献库（查重、key 规则与 DOI 导入一致）
fn zotero_add(ws: &mut Workspace, p: Value) -> CommandResult {
    let KeysParam { keys } = params(p)?;
    let entries = ym_bib::zotero::export(&keys).map_err(|e| CommandError::Failed(e.to_string()))?;
    let mut added = Vec::new();
    for e in &entries {
        added.push(bib_add(
            ws,
            json!({ "bibtex": ym_bib::import::to_bibtex(e) }),
        )?);
        ws.invalidate();
    }
    Ok(json!({ "added": added }))
}

fn bib_abstracts(ws: &mut Workspace, p: Value) -> CommandResult {
    let KeysParam { keys } = params(p)?;
    let entries = ws.bib_entries(&keys)?;
    Ok(crate::fetch_abstracts(&entries))
}

fn bib_verify(ws: &mut Workspace, p: Value) -> CommandResult {
    let KeysParam { keys } = if p.is_null() {
        KeysParam::default()
    } else {
        params(p)?
    };
    let entries = ws.bib_entries(&keys)?;
    Ok(crate::verify_bib(&entries))
}

#[derive(Deserialize)]
struct MoveParam {
    id: String,
    target: String,
    position: ym_latex::reorder::Position,
}

/// 移动章节：只重写一个文件，移动的文本原样保留。
fn outline_move(ws: &mut Workspace, p: Value) -> CommandResult {
    let MoveParam {
        id,
        target,
        position,
    } = params(p)?;
    ws.invalidate();
    let index = ws.index()?;
    let plan = ym_latex::reorder::plan_move(index, &id, &target, position)
        .map_err(|e| CommandError::Failed(e.to_string()))?;
    let changed = plan.new_text != plan.old_text;
    if changed {
        fs::write(ws.resolve(&plan.file.to_string_lossy())?, &plan.new_text)?;
        ws.invalidate();
    }
    Ok(json!({ "file": plan.file, "changed": changed }))
}

#[derive(Deserialize)]
struct LangParam {
    lang: ym_model::Lang,
}

/// 设置后端消息（检查结果、错误提示等）使用的语言。
fn app_set_lang(_: &mut Workspace, p: Value) -> CommandResult {
    let LangParam { lang } = params(p)?;
    ym_model::set_lang(lang);
    Ok(json!({ "lang": lang }))
}

fn bib_fetch(_: &mut Workspace, p: Value) -> CommandResult {
    let IdParam { id } = params(p)?;
    let ident = ym_bib::import::detect(&id).ok_or_else(|| {
        CommandError::InvalidParams(tr!(
            "无法识别为 DOI 或 arXiv 编号：{id}",
            "Not a DOI or arXiv ID: {id}"
        ))
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
    let mut entry = parsed.entries.into_iter().next().ok_or_else(|| {
        CommandError::InvalidParams(tr!("没有可识别的 BibTeX 条目", "No BibTeX entry found"))
    })?;

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
        _ => {
            return Err(CommandError::InvalidParams(tr!(
                "需要 source 或 base64",
                "source or base64 is required"
            )))
        }
    };
    let fname = Path::new(&file_name);
    let ext = fname
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !EXTS.contains(&ext.as_str()) {
        return Err(CommandError::InvalidParams(tr!(
            "LaTeX 不能直接插入 .{ext} 文件，支持 PNG、JPG、PDF、EPS",
            "LaTeX cannot include .{ext} files directly; use PNG, JPG, PDF or EPS"
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
    /// 超时（秒）
    timeout_secs: Option<u64>,
}

fn compile_run(ws: &mut Workspace, p: Value) -> CommandResult {
    let CompileParam {
        engine,
        timeout_secs,
    } = if p.is_null() {
        CompileParam::default()
    } else {
        params(p)?
    };
    let job = ws.compile_job(engine, timeout_secs.map(std::time::Duration::from_secs))?;
    let out = job.run()?;
    ws.finish_compile(out.clone());
    to_value(&out)
}

fn compile_cancel(ws: &mut Workspace, _: Value) -> CommandResult {
    Ok(json!({ "cancelled": ws.cancel_compile() }))
}

fn compile_clean(ws: &mut Workspace, _: Value) -> CommandResult {
    let root = ws.root()?.to_path_buf();
    ym_compile::clean(&root)?;
    ws.finish_compile_cleared();
    Ok(json!({ "cleaned": ym_compile::BUILD_DIR }))
}

#[derive(Deserialize, Default)]
struct SettingsParam {
    /// "auto" 表示自动选择
    engine: Option<String>,
    shell_escape: Option<bool>,
}

fn settings_json(ws: &Workspace) -> Value {
    let meta = ws.meta();
    json!({
        "engine": meta.engine.map(|e| serde_json::to_value(e).unwrap_or(Value::Null)).unwrap_or(json!("auto")),
        "shell_escape": meta.shell_escape,
        "main": ws.main(),
    })
}

#[derive(Deserialize, Default)]
struct ExportZipParam {
    output: Option<String>,
}

fn project_export_zip(ws: &mut Workspace, p: Value) -> CommandResult {
    let ExportZipParam { output } = if p.is_null() {
        ExportZipParam::default()
    } else {
        params(p)?
    };
    let root = ws.root()?.to_path_buf();
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "paper".into());
    let rel = output.unwrap_or_else(|| format!("{}/{name}.zip", ym_compile::EXPORT_DIR));
    let out = ws.resolve(&rel)?;
    let main_source = ws
        .main()
        .and_then(|m| fs::read_to_string(root.join(m)).ok())
        .unwrap_or_default();
    let rc = crate::overleaf::latexmkrc_for(ws.meta().engine, &main_source);
    let r = crate::overleaf::export_zip(&root, &out, rc)?;
    Ok(json!({
        "path": rel,
        "files": r.files,
        "bytes": r.bytes,
        "added_latexmkrc": r.added_latexmkrc,
        "omitted": r.omitted,
        // Overleaf 单次上传的上限
        "too_large": r.bytes > 50 * 1024 * 1024,
    }))
}

#[derive(Deserialize)]
struct ImportZipParam {
    zip: PathBuf,
    parent: PathBuf,
    name: Option<String>,
}

fn project_import_zip(_: &mut Workspace, p: Value) -> CommandResult {
    let ImportZipParam { zip, parent, name } = params(p)?;
    if !zip.is_file() {
        return Err(CommandError::InvalidParams(tr!(
            "找不到文件：{p}",
            "File not found: {p}",
            p = zip.display()
        )));
    }
    if !parent.is_absolute() || !parent.is_dir() {
        return Err(CommandError::InvalidParams(tr!(
            "不是文件夹：{p}",
            "Not a folder: {p}",
            p = parent.display()
        )));
    }
    let explicit = name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_owned);
    let name = explicit
        .clone()
        .or_else(|| zip.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "paper".into());
    if name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(CommandError::InvalidParams(tr!(
            "项目名称不能包含 / 或以 . 开头",
            "The project name must not contain / or start with ."
        )));
    }
    // 未指定名称且默认名称已被占用时，加数字后缀
    let mut dest = parent.join(&name);
    let mut n = 2;
    while explicit.is_none() && dest.exists() {
        dest = parent.join(format!("{name}-{n}"));
        n += 1;
    }
    let files = crate::overleaf::import_zip(&zip, &dest)?;
    Ok(json!({ "path": dest, "files": files }))
}

fn project_templates(_: &mut Workspace, _: Value) -> CommandResult {
    to_value(&crate::templates::TEMPLATES)
}

#[derive(Deserialize)]
struct CreateParam {
    template: String,
    parent: PathBuf,
    name: String,
    title: Option<String>,
}

/// 转义 LaTeX 特殊字符，用于把用户输入的标题写进模板
fn latex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => {
                out.push('\\');
                out.push(c);
            }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\\' => out.push_str("\\textbackslash{}"),
            c => out.push(c),
        }
    }
    out
}

fn project_create(_: &mut Workspace, p: Value) -> CommandResult {
    let CreateParam {
        template,
        parent,
        name,
        title,
    } = params(p)?;
    let t = crate::templates::find(&template).ok_or_else(|| {
        CommandError::InvalidParams(tr!(
            "没有这个模板：{template}",
            "No such template: {template}"
        ))
    })?;
    let name = name.trim();
    if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
        return Err(CommandError::InvalidParams(tr!(
            "项目名称不能为空，也不能包含 / 或以 . 开头",
            "The project name must not be empty, contain / or start with ."
        )));
    }
    if !parent.is_absolute() || !parent.is_dir() {
        return Err(CommandError::InvalidParams(tr!(
            "不是文件夹：{p}",
            "Not a folder: {p}",
            p = parent.display()
        )));
    }
    let dir = parent.join(name);
    if dir.exists() {
        return Err(CommandError::Failed(tr!(
            "{p} 已存在，请换一个名称",
            "{p} already exists; choose another name",
            p = dir.display()
        )));
    }
    let title = latex_escape(
        title
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(name),
    );
    for (rel, text) in t.files {
        let path = dir.join(rel);
        if let Some(d) = path.parent() {
            fs::create_dir_all(d)?;
        }
        fs::write(path, text.replace("{{title}}", &title))?;
    }
    Ok(json!({ "path": dir }))
}

fn project_settings(ws: &mut Workspace, _: Value) -> CommandResult {
    ws.root()?;
    Ok(settings_json(ws))
}

fn project_update_settings(ws: &mut Workspace, p: Value) -> CommandResult {
    let SettingsParam {
        engine,
        shell_escape,
    } = params(p)?;
    let engine = match engine.as_deref() {
        None => None,
        Some("auto") => Some(None),
        Some(e) => Some(Some(
            serde_json::from_value::<ym_model::Engine>(json!(e))
                .map_err(|err| CommandError::InvalidParams(err.to_string()))?,
        )),
    };
    ws.update_meta(|m| {
        if let Some(e) = engine {
            m.engine = e;
        }
        if let Some(s) = shell_escape {
            m.shell_escape = s;
        }
    })?;
    Ok(settings_json(ws))
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
    let rel = ws.pdf_path().ok_or_else(|| {
        CommandError::Failed(tr!("尚未编译出 PDF", "No PDF has been compiled yet"))
    })?;
    let bytes = fs::read(ws.root()?.join(&rel))?;
    Ok(json!({ "path": rel, "base64": base64::engine::general_purpose::STANDARD.encode(bytes) }))
}

#[derive(Deserialize, Default)]
struct ToolchainParam {
    #[serde(default)]
    refresh: bool,
}

fn compile_toolchain(ws: &mut Workspace, p: Value) -> CommandResult {
    let ToolchainParam { refresh } = if p.is_null() {
        ToolchainParam::default()
    } else {
        params(p)?
    };
    if refresh {
        to_value(ws.redetect_toolchain())
    } else {
        to_value(ws.toolchain())
    }
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
