//! 项目级索引：从主文件出发沿 `\input`/`\include` 递归，拼装跨文件大纲。

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::{fs, io};
use ym_model::{Asset, Section, SectionLevel, SourceSpan};

use crate::{parse_file, FileIndex, FlowItem, Heading, Located};

/// `\includegraphics` 省略扩展名时依次尝试的扩展名。
pub const GRAPHIC_EXTS: &[&str] = &["pdf", "png", "jpg", "jpeg", "eps", "PDF", "PNG", "JPG"];

#[derive(Debug, Serialize)]
pub struct ProjectIndex {
    pub root: PathBuf,
    /// 主文件，相对 `root`
    pub main: PathBuf,
    /// 按文档顺序访问到的文件
    pub files: Vec<FileIndex>,
    pub outline: Vec<Section>,
    /// 第一个章节标题之前的正文字数（摘要等）
    pub front_words: u32,
    /// 找不到目标文件的 `\input`/`\include`
    pub unresolved_includes: Vec<Located>,
}

impl ProjectIndex {
    /// 解析 `\input`、`\includegraphics` 等相对路径时的基准目录（主文件所在目录，相对 `root`）。
    pub fn base_dir(&self) -> PathBuf {
        self.main
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default()
    }

    pub fn labels(&self) -> impl Iterator<Item = &Located> {
        self.files.iter().flat_map(|f| &f.labels)
    }

    pub fn refs(&self) -> impl Iterator<Item = &Located> {
        self.files.iter().flat_map(|f| &f.refs)
    }

    pub fn cites(&self) -> impl Iterator<Item = &Located> {
        self.files.iter().flat_map(|f| &f.cites)
    }

    pub fn bibitems(&self) -> impl Iterator<Item = &Located> {
        self.files.iter().flat_map(|f| &f.bibitems)
    }

    pub fn bib_files(&self) -> impl Iterator<Item = &Located> {
        self.files.iter().flat_map(|f| &f.bib_files)
    }

    pub fn graphics(&self) -> impl Iterator<Item = &Located> {
        self.files.iter().flat_map(|f| &f.graphics)
    }

    pub fn graphics_paths(&self) -> impl Iterator<Item = &str> {
        self.files
            .iter()
            .flat_map(|f| &f.graphics_paths)
            .map(String::as_str)
    }

    /// `\includegraphics{name}` 实际对应的项目内文件（相对 `root`）：依次在主文件目录与
    /// `\graphicspath` 中查找，省略扩展名时按 pdfLaTeX/XeLaTeX 的默认顺序补全。
    pub fn resolve_graphic(&self, name: &str) -> Option<PathBuf> {
        let base = self.base_dir();
        let dirs = std::iter::once(PathBuf::new()).chain(self.graphics_paths().map(PathBuf::from));
        for dir in dirs {
            let p = base.join(dir).join(name);
            let mut candidates = vec![p.clone()];
            if p.extension().is_none() {
                candidates.extend(GRAPHIC_EXTS.iter().map(|e| p.with_extension(e)));
            }
            if let Some(found) = candidates
                .into_iter()
                .map(|c| normalize(&c))
                .find(|c| self.root.join(c).is_file())
            {
                return Some(found);
            }
        }
        None
    }

    pub fn assets(&self) -> impl Iterator<Item = &Asset> {
        self.files.iter().flat_map(|f| &f.assets)
    }

    pub fn nocite_all(&self) -> bool {
        self.files.iter().any(|f| f.nocite_all)
    }

    pub fn total_words(&self) -> u32 {
        fn sum(sections: &[Section]) -> u32 {
            sections.iter().map(|s| s.words + sum(&s.children)).sum()
        }
        self.front_words + sum(&self.outline)
    }
}

/// 一个目录里可以有多个独立文档（正文、补充材料、标题页……），全部列出，相对 `root`。
/// 只看根目录；根目录没有时再看一级子目录。
pub fn find_roots(root: &Path) -> Vec<PathBuf> {
    let is_root_doc = |p: &Path| {
        fs::read_to_string(p).is_ok_and(|s| {
            s.lines()
                .any(|l| l.trim_start().starts_with("\\documentclass"))
        })
    };
    let list = |dir: &Path| -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "tex") && is_root_doc(p))
            .filter_map(|p| p.strip_prefix(root).ok().map(Path::to_path_buf))
            .collect();
        files.sort();
        files
    };
    let found = list(root);
    if !found.is_empty() {
        return found;
    }
    let mut subdirs: Vec<PathBuf> = fs::read_dir(root)
        .map(|e| {
            e.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir() && !is_hidden(p))
                .collect()
        })
        .unwrap_or_default();
    subdirs.sort();
    subdirs.iter().flat_map(|d| list(d)).collect()
}

/// 在多个独立文档中挑出主文件：常见的主文件名优先（main / manuscript / paper / thesis …），
/// 其次是篇幅最长的文档；标题页、亮点、补充材料、cover letter 之类排在后面。
pub fn find_main(root: &Path) -> Option<PathBuf> {
    const PREFERRED: &[&str] = &[
        "main",
        "manuscript",
        "paper",
        "thesis",
        "article",
        "dissertation",
    ];
    const SECONDARY: &[&str] = &[
        "supplement",
        "appendix",
        "title",
        "highlight",
        "cover",
        "letter",
        "response",
        "rebuttal",
    ];
    let score = |p: &PathBuf| {
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let size = fs::metadata(root.join(p)).map(|m| m.len()).unwrap_or(0);
        let rank = if PREFERRED.contains(&stem.as_str()) {
            3
        } else if PREFERRED.iter().any(|n| stem.starts_with(n)) {
            2
        } else if SECONDARY.iter().any(|n| stem.contains(n)) {
            0
        } else {
            1
        };
        (rank, size)
    };
    find_roots(root).into_iter().max_by_key(score)
}

fn is_hidden(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// 去掉 `./` 并消解 `..`，得到可比较的相对路径。
pub(crate) fn normalize(p: &Path) -> PathBuf {
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

/// 索引整个项目。`main` 为相对 `root` 的主文件路径。
pub fn index_project(root: &Path, main: &Path) -> io::Result<ProjectIndex> {
    let main = normalize(main.strip_prefix(root).unwrap_or(main));
    let mut walker = Walker {
        root,
        base: main.parent().map(Path::to_path_buf).unwrap_or_default(),
        files: Vec::new(),
        visited: HashSet::new(),
        flat: Vec::new(),
        front_words: 0,
        unresolved: Vec::new(),
    };
    let src = fs::read_to_string(root.join(&main))?;
    walker.walk(main.clone(), &src, true);

    let mut ids = HashMap::new();
    let mut i = 0;
    let outline = build_outline(&walker.flat, &mut i, None, "", &mut ids);

    let mut files = walker.files;
    link_asset_references(&mut files);

    Ok(ProjectIndex {
        root: root.to_path_buf(),
        main,
        files,
        outline,
        front_words: walker.front_words,
        unresolved_includes: walker.unresolved,
    })
}

struct Walker<'r> {
    root: &'r Path,
    base: PathBuf,
    files: Vec<FileIndex>,
    visited: HashSet<PathBuf>,
    /// 按文档顺序排列的 (标题, 本节自身字数)
    flat: Vec<(Heading, u32)>,
    front_words: u32,
    unresolved: Vec<Located>,
}

impl Walker<'_> {
    fn walk(&mut self, rel: PathBuf, src: &str, in_body: bool) {
        if !self.visited.insert(rel.clone()) {
            return; // 防止循环包含
        }
        let index = parse_file(&rel, src);
        let flow = index.flow.clone();
        let headings = index.headings.clone();
        let includes = index.includes.clone();
        self.files.push(index);

        for item in flow {
            match item {
                FlowItem::Words(n) if in_body => match self.flat.last_mut() {
                    Some((_, words)) => *words += n,
                    None => self.front_words += n,
                },
                FlowItem::Heading(i) if in_body => self.flat.push((headings[i].clone(), 0)),
                FlowItem::Include(i) | FlowItem::PreambleInclude(i) => {
                    let include = &includes[i];
                    let child_in_body = in_body && matches!(item, FlowItem::Include(_));
                    match self.resolve(&include.name) {
                        Some(path) => match fs::read_to_string(self.root.join(&path)) {
                            Ok(child) => self.walk(path, &child, child_in_body),
                            Err(_) => self.unresolved.push(include.clone()),
                        },
                        None => self.unresolved.push(include.clone()),
                    }
                }
                _ => {}
            }
        }
    }

    /// 与 LaTeX 一致：相对主文件目录解析，未写 `.tex` 扩展名时优先补全。
    fn resolve(&self, target: &str) -> Option<PathBuf> {
        let rel = self.base.join(target);
        let mut candidates = Vec::new();
        if rel.extension().is_none_or(|e| e != "tex") {
            candidates.push(PathBuf::from(format!("{}.tex", rel.display())));
        }
        candidates.push(rel);
        candidates
            .into_iter()
            .map(|c| normalize(&c))
            .find(|c| self.root.join(c).is_file())
    }
}

fn build_outline(
    flat: &[(Heading, u32)],
    i: &mut usize,
    parent: Option<SectionLevel>,
    parent_id: &str,
    ids: &mut HashMap<String, u32>,
) -> Vec<Section> {
    let mut out = Vec::new();
    while *i < flat.len() && parent.is_none_or(|p| flat[*i].0.level > p) {
        let (heading, words) = &flat[*i];
        *i += 1;
        let base_id = heading
            .label
            .clone()
            .unwrap_or_else(|| format!("{parent_id}/{}", heading.title));
        let seen = ids.entry(base_id.clone()).or_insert(0);
        let id = if *seen == 0 {
            base_id
        } else {
            format!("{base_id}#{seen}")
        };
        *seen += 1;
        let children = build_outline(flat, i, Some(heading.level), &id, ids);
        out.push(Section {
            id,
            level: heading.level,
            title: heading.title.clone(),
            label: heading.label.clone(),
            span: heading.span.clone(),
            words: *words,
            children,
        });
    }
    out
}

fn link_asset_references(files: &mut [FileIndex]) {
    let mut refs: HashMap<String, Vec<SourceSpan>> = HashMap::new();
    for r in files.iter().flat_map(|f| &f.refs) {
        refs.entry(r.name.clone()).or_default().push(r.span.clone());
    }
    for asset in files.iter_mut().flat_map(|f| &mut f.assets) {
        if let Some(label) = &asset.label {
            asset.references = refs.get(label).cloned().unwrap_or_default();
        }
    }
}
