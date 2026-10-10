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
    r.register(meta("check.run", "检查项目问题", none, false), check_run);
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

fn project_tree(ws: &mut Workspace, _: Value) -> CommandResult {
    let root = ws.root()?.to_path_buf();
    to_value(&walk(&root, Path::new(""), 6))
}

fn project_index(ws: &mut Workspace, _: Value) -> CommandResult {
    let index = ws.index()?;
    let assets: Vec<_> = index.assets().collect();
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
    Ok(json!({
        "errors": report.count(ym_check::Severity::Error),
        "warnings": report.count(ym_check::Severity::Warning),
        "issues": issues,
        "bib": report.bib,
    }))
}

fn file_read(ws: &mut Workspace, p: Value) -> CommandResult {
    let PathParam { path } = params(p)?;
    let text = fs::read_to_string(ws.resolve(&path)?)?;
    Ok(json!({ "path": path, "text": text }))
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
