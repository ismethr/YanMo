//! 项目一致性检查，供问题面板与 `ym check` 使用。

use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use ym_latex::ProjectIndex;
use ym_model::{AssetKind, BibEntry, SourceSpan};

/// `\includegraphics` 省略扩展名时依次尝试的扩展名（与 pdfLaTeX/XeLaTeX 默认顺序一致）。
use ym_latex::GRAPHIC_EXTS;
/// 未指定 `\graphicspath` 时也会检查的常见图片目录。
const COMMON_FIGURE_DIRS: &[&str] = &["figures", "figure", "figs", "fig", "images", "img"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Issue {
    UndefinedRef {
        label: String,
        at: SourceSpan,
    },
    DuplicateLabel {
        label: String,
        at: Vec<SourceSpan>,
    },
    /// 图、表、算法定义了 label 却从未被引用
    UnreferencedAsset {
        label: String,
        kind: AssetKind,
        at: SourceSpan,
    },
    UndefinedCite {
        key: String,
        at: SourceSpan,
    },
    UnusedBibEntry {
        key: String,
        at: SourceSpan,
    },
    DuplicateBibEntry {
        key: String,
        at: Vec<SourceSpan>,
    },
    BibSyntax {
        file: PathBuf,
        message: String,
    },
    /// `\input`、`\includegraphics`、`\bibliography` 指向的文件不存在
    MissingFile {
        path: String,
        at: SourceSpan,
    },
    /// 图片目录里有文件但正文从未使用
    UnusedFigureFile {
        path: PathBuf,
    },
}

impl Issue {
    pub fn severity(&self) -> Severity {
        match self {
            Issue::UndefinedRef { .. }
            | Issue::DuplicateLabel { .. }
            | Issue::UndefinedCite { .. }
            | Issue::DuplicateBibEntry { .. }
            | Issue::BibSyntax { .. }
            | Issue::MissingFile { .. } => Severity::Error,
            Issue::UnreferencedAsset { .. }
            | Issue::UnusedBibEntry { .. }
            | Issue::UnusedFigureFile { .. } => Severity::Warning,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Issue::UndefinedRef { .. } => "undefined_ref",
            Issue::DuplicateLabel { .. } => "duplicate_label",
            Issue::UnreferencedAsset { .. } => "unreferenced_asset",
            Issue::UndefinedCite { .. } => "undefined_cite",
            Issue::UnusedBibEntry { .. } => "unused_bib_entry",
            Issue::DuplicateBibEntry { .. } => "duplicate_bib_entry",
            Issue::BibSyntax { .. } => "bib_syntax",
            Issue::MissingFile { .. } => "missing_file",
            Issue::UnusedFigureFile { .. } => "unused_figure_file",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Issue::UndefinedRef { label, .. } => format!("引用了未定义的 label `{label}`"),
            Issue::DuplicateLabel { label, at } => {
                format!("label `{label}` 被定义了 {} 次", at.len())
            }
            Issue::UnreferencedAsset { label, kind, .. } => {
                format!("{} `{label}` 从未在正文中被引用", kind_name(*kind))
            }
            Issue::UndefinedCite { key, .. } => format!("引用了不存在的文献 `{key}`"),
            Issue::UnusedBibEntry { key, .. } => format!("文献 `{key}` 从未被引用"),
            Issue::DuplicateBibEntry { key, at } => {
                format!("文献 key `{key}` 重复出现 {} 次", at.len())
            }
            Issue::BibSyntax { message, .. } => format!("bib 语法错误：{message}"),
            Issue::MissingFile { path, .. } => format!("找不到文件 `{path}`"),
            Issue::UnusedFigureFile { path } => {
                format!("图片 `{}` 未被任何 \\includegraphics 使用", path.display())
            }
        }
    }

    /// 首要定位位置（文件, 行号），用于展示与跳转。
    pub fn location(&self) -> (Option<&Path>, Option<u32>) {
        let span = match self {
            Issue::UndefinedRef { at, .. }
            | Issue::UnreferencedAsset { at, .. }
            | Issue::UndefinedCite { at, .. }
            | Issue::UnusedBibEntry { at, .. }
            | Issue::MissingFile { at, .. } => Some(at),
            Issue::DuplicateLabel { at, .. } | Issue::DuplicateBibEntry { at, .. } => at.get(1),
            Issue::BibSyntax { file, .. } => return (Some(file), None),
            Issue::UnusedFigureFile { path } => return (Some(path), None),
        };
        (span.map(|s| s.file.as_path()), span.map(|s| s.line))
    }
}

fn kind_name(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Figure => "图",
        AssetKind::Table => "表",
        AssetKind::Equation => "公式",
        AssetKind::Algorithm => "算法",
        AssetKind::Listing => "代码",
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub issues: Vec<Issue>,
    /// 项目实际使用的文献（已填充 `citations`）
    pub bib: Vec<BibEntry>,
}

impl Report {
    pub fn count(&self, severity: Severity) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity() == severity)
            .count()
    }
}

/// 读取文献并执行全部检查。
pub fn run(project: &ProjectIndex) -> Report {
    let (mut bib, mut issues) = load_bib(project);
    issues.extend(check_with(project, &bib, &SiblingUsage::collect(project)));
    attach_citations(project, &mut bib);
    issues.sort_by(|a, b| (a.severity(), a.location()).cmp(&(b.severity(), b.location())));
    Report { issues, bib }
}

/// 读取项目引用的所有 `.bib` 文件。
pub fn load_bib(project: &ProjectIndex) -> (Vec<BibEntry>, Vec<Issue>) {
    let base = project.base_dir();
    let mut entries = Vec::new();
    let mut issues = Vec::new();
    let mut seen = HashSet::new();
    for file in project.bib_files() {
        let name = if file.name.ends_with(".bib") {
            file.name.clone()
        } else {
            format!("{}.bib", file.name)
        };
        let rel = normalize(&base.join(&name));
        if !seen.insert(rel.clone()) {
            continue;
        }
        match fs::read_to_string(project.root.join(&rel)) {
            Ok(src) => {
                let out = ym_bib::parse(&rel, &src);
                entries.extend(out.entries);
                issues.extend(out.errors.into_iter().map(|e| Issue::BibSyntax {
                    file: rel.clone(),
                    message: e.to_string(),
                }));
            }
            Err(_) => issues.push(Issue::MissingFile {
                path: name,
                at: file.span.clone(),
            }),
        }
    }
    (entries, issues)
}

/// 纯逻辑检查（图片相关检查会访问文件系统）。
pub fn check(project: &ProjectIndex, bib: &[BibEntry]) -> Vec<Issue> {
    check_with(project, bib, &SiblingUsage::default())
}

/// 同一目录下其他独立文档（补充材料、标题页等）的用法。
/// 它们与主文档共用图片目录和 .bib，判断"未使用"时要一并考虑。
#[derive(Debug, Default)]
pub struct SiblingUsage {
    pub cites: HashSet<String>,
    pub nocite_all: bool,
    /// 图片文件的比较键，见 [`file_key`]
    pub graphics: HashSet<String>,
    /// 参与统计的文档，相对项目根目录
    pub documents: Vec<PathBuf>,
}

impl SiblingUsage {
    /// 索引项目根目录中除主文件外的其他独立文档。
    pub fn collect(project: &ProjectIndex) -> Self {
        let mut usage = SiblingUsage::default();
        for doc in ym_latex::find_roots(&project.root) {
            if doc == project.main {
                continue;
            }
            let Ok(other) = ym_latex::index_project(&project.root, &doc) else {
                continue;
            };
            usage.cites.extend(other.cites().map(|c| c.name.clone()));
            usage.nocite_all |= other.nocite_all();
            for (_, found) in resolve_graphics(&other) {
                if let Some(p) = found {
                    usage.graphics.insert(file_key(&project.root, &p));
                }
            }
            usage.documents.push(doc);
        }
        usage
    }
}

/// 文件的比较键：macOS 与 Windows 的文件系统默认不区分大小写，
/// `Figures/a.pdf` 与 `figures/a.pdf` 是同一个文件。
pub fn file_key(root: &Path, rel: &Path) -> String {
    let abs = root.join(rel);
    let p = abs.canonicalize().unwrap_or(abs);
    let s = p.to_string_lossy().into_owned();
    if cfg!(any(target_os = "macos", windows)) {
        s.to_lowercase()
    } else {
        s
    }
}

pub fn check_with(project: &ProjectIndex, bib: &[BibEntry], siblings: &SiblingUsage) -> Vec<Issue> {
    let mut issues = Vec::new();

    // ---- label / ref ----
    let mut labels: BTreeMap<&str, Vec<SourceSpan>> = BTreeMap::new();
    for l in project.labels() {
        labels.entry(&l.name).or_default().push(l.span.clone());
    }
    for (label, at) in &labels {
        if at.len() > 1 {
            issues.push(Issue::DuplicateLabel {
                label: label.to_string(),
                at: at.clone(),
            });
        }
    }
    for r in project.refs() {
        if !labels.contains_key(r.name.as_str()) {
            issues.push(Issue::UndefinedRef {
                label: r.name.clone(),
                at: r.span.clone(),
            });
        }
    }
    for asset in project.assets() {
        let reportable = matches!(
            asset.kind,
            AssetKind::Figure | AssetKind::Table | AssetKind::Algorithm | AssetKind::Listing
        );
        if let (true, Some(label), true) = (reportable, &asset.label, asset.references.is_empty()) {
            issues.push(Issue::UnreferencedAsset {
                label: label.clone(),
                kind: asset.kind,
                at: asset.span.clone(),
            });
        }
    }

    // ---- cite / bib ----
    let mut keys: BTreeMap<&str, Vec<SourceSpan>> = BTreeMap::new();
    for e in bib {
        keys.entry(&e.key).or_default().push(e.span.clone());
    }
    let bibitems: HashSet<&str> = project.bibitems().map(|b| b.name.as_str()).collect();
    let cited: HashSet<&str> = project.cites().map(|c| c.name.as_str()).collect();
    for c in project.cites() {
        if !keys.contains_key(c.name.as_str()) && !bibitems.contains(c.name.as_str()) {
            issues.push(Issue::UndefinedCite {
                key: c.name.clone(),
                at: c.span.clone(),
            });
        }
    }
    for (key, at) in &keys {
        if at.len() > 1 {
            issues.push(Issue::DuplicateBibEntry {
                key: key.to_string(),
                at: at.clone(),
            });
        }
        let used_elsewhere = siblings.nocite_all || siblings.cites.contains(*key);
        if !project.nocite_all() && !cited.contains(key) && !used_elsewhere {
            issues.push(Issue::UnusedBibEntry {
                key: key.to_string(),
                at: at[0].clone(),
            });
        }
    }

    // ---- 文件 ----
    for inc in &project.unresolved_includes {
        issues.push(Issue::MissingFile {
            path: inc.name.clone(),
            at: inc.span.clone(),
        });
    }
    issues.extend(check_graphics(project, siblings));
    issues
}

/// 解析每条 `\includegraphics` 实际对应的文件（相对项目根目录）。
fn resolve_graphics(project: &ProjectIndex) -> Vec<(&ym_latex::Located, Option<PathBuf>)> {
    project
        .graphics()
        .map(|g| (g, project.resolve_graphic(&g.name)))
        .collect()
}

fn check_graphics(project: &ProjectIndex, siblings: &SiblingUsage) -> Vec<Issue> {
    let base = project.base_dir();
    let mut issues = Vec::new();
    let mut used: Vec<PathBuf> = Vec::new();
    for (g, found) in resolve_graphics(project) {
        match found {
            Some(p) => used.push(p),
            None => issues.push(Issue::MissingFile {
                path: g.name.clone(),
                at: g.span.clone(),
            }),
        }
    }
    let mut used_keys: HashSet<String> = used.iter().map(|p| file_key(&project.root, p)).collect();
    used_keys.extend(siblings.graphics.iter().cloned());

    // 图片目录：\graphicspath、已使用图片所在目录、常见目录名；按实际目录去重（不区分大小写的文件系统上
    // Figures/ 与 figures/ 是同一个目录）
    let mut candidates: Vec<PathBuf> = project
        .graphics_paths()
        .map(|d| normalize(&base.join(d)))
        .collect();
    candidates.extend(
        used.iter()
            .filter_map(|p| p.parent().map(Path::to_path_buf)),
    );
    candidates.extend(COMMON_FIGURE_DIRS.iter().map(|d| normalize(&base.join(d))));
    let mut seen = HashSet::new();
    let dirs: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|d| !d.as_os_str().is_empty() && project.root.join(d).is_dir())
        .filter(|d| seen.insert(file_key(&project.root, d)))
        .collect();

    // 同目录同名、仅扩展名不同的文件视为同一张图的其他格式（如给 Word 准备的 PNG/SVG），不提示
    let stem_key = |k: &str| match k.rfind('.') {
        Some(i) if !k[i..].contains('/') => k[..i].to_owned(),
        _ => k.to_owned(),
    };
    let used_stems: HashSet<String> = used_keys.iter().map(|k| stem_key(k)).collect();

    let mut unused: Vec<PathBuf> = Vec::new();
    let mut reported = HashSet::new();
    for dir in &dirs {
        collect_images(&project.root, dir, 3, &mut |p| {
            let key = file_key(&project.root, &p);
            let variant = used_stems.contains(&stem_key(&key));
            if !used_keys.contains(&key) && !variant && reported.insert(key) {
                unused.push(p);
            }
        });
    }
    unused.sort();
    issues.extend(
        unused
            .into_iter()
            .map(|path| Issue::UnusedFigureFile { path }),
    );
    issues
}

fn collect_images(root: &Path, dir: &Path, depth: u32, f: &mut dyn FnMut(PathBuf)) {
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let rel = dir.join(&name);
        let path = e.path();
        if path.is_dir() {
            if depth > 0 {
                collect_images(root, &rel, depth - 1, f);
            }
        } else if rel
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| GRAPHIC_EXTS.contains(&e) || e.eq_ignore_ascii_case("svg"))
        {
            f(normalize(&rel));
        }
    }
}

fn attach_citations(project: &ProjectIndex, bib: &mut [BibEntry]) {
    let mut by_key: HashMap<&str, Vec<SourceSpan>> = HashMap::new();
    for c in project.cites() {
        by_key.entry(&c.name).or_default().push(c.span.clone());
    }
    for e in bib {
        e.citations = by_key.get(e.key.as_str()).cloned().unwrap_or_default();
    }
}

fn normalize(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_paper_issues() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample-paper");
        let project = ym_latex::index_project(&root, Path::new("main.tex")).unwrap();
        let report = run(&project);

        let mut found: Vec<(&str, String)> = report
            .issues
            .iter()
            .map(|i| (i.code(), i.message()))
            .collect();
        found.sort();
        let codes: Vec<&str> = found.iter().map(|(c, _)| *c).collect();
        assert_eq!(
            codes,
            [
                "duplicate_label",
                "missing_file",
                "undefined_cite",
                "undefined_ref",
                "unreferenced_asset",
                "unused_bib_entry",
            ],
            "{found:#?}"
        );
        assert_eq!(report.count(Severity::Error), 4);

        let undefined = report
            .issues
            .iter()
            .find(|i| i.code() == "undefined_ref")
            .unwrap();
        assert_eq!(
            undefined.location(),
            (Some(Path::new("sections/intro.tex")), Some(3))
        );
        let vaswani = report
            .bib
            .iter()
            .find(|e| e.key == "vaswani2017attention")
            .unwrap();
        assert_eq!(vaswani.citations.len(), 1);
    }
}

#[cfg(test)]
mod sibling_tests {
    use super::*;

    #[test]
    fn usage_in_sibling_documents_is_not_reported_as_unused() {
        let dir = std::env::temp_dir().join(format!("ym-siblings-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Figures")).unwrap();
        for f in ["main.pdf", "suppl.pdf", "main.png", "orphan.pdf"] {
            std::fs::write(dir.join("Figures").join(f), b"x").unwrap();
        }
        std::fs::write(
            dir.join("refs.bib"),
            "@article{a,title={A}}\n@article{b,title={B}}\n@article{c,title={C}}\n",
        )
        .unwrap();
        let doc = |body: &str| {
            format!("\\documentclass{{article}}\n\\begin{{document}}\n{body}\n\\bibliography{{refs}}\n\\end{{document}}\n")
        };
        std::fs::write(
            dir.join("manuscript.tex"),
            doc("\\cite{a}\\includegraphics{Figures/main.pdf}"),
        )
        .unwrap();
        std::fs::write(
            dir.join("supplement.tex"),
            // 不区分大小写的文件系统上用小写目录名，验证 Figures/ 与 figures/ 被视为同一目录
            doc(if cfg!(any(target_os = "macos", windows)) {
                "\\cite{b}\\includegraphics{figures/suppl.pdf}"
            } else {
                "\\cite{b}\\includegraphics{Figures/suppl.pdf}"
            }),
        )
        .unwrap();

        let project = ym_latex::index_project(&dir, Path::new("manuscript.tex")).unwrap();
        let report = run(&project);
        let found: Vec<String> = report.issues.iter().map(|i| i.message()).collect();
        // b 被补充材料引用、suppl.pdf 被补充材料使用（大小写不同的目录名）、main.png 是 main.pdf 的另一格式
        assert_eq!(found.len(), 2, "{found:#?}");
        assert!(found.iter().any(|m| m.contains("`c`")));
        assert!(found.iter().any(|m| m.contains("orphan.pdf")));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
