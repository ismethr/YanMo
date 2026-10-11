//! 移动章节：算出移动后的文件内容（只改动一个文件，移动的文本原样保留）。
//!
//! 章节的范围从标题命令开始，到文档顺序中下一个同级或更高级的标题为止；
//! 遇到 `\appendix`、参考文献、`\end{document}` 等位置也停止，不把它们一并带走。
//!
//! 章节常被拆成独立文件再用 `\input` 引入。若一节恰好占满被引入的文件（标题前只有空白和注释），
//! 就把范围"提升"为父文件中的那条 `\input` 命令；插入位置同样可以提升。
//! 在章节范围和插入位置共同所在的最深一层文件里完成移动；找不到这样的文件
//! （例如要插到另一个文件的中间）就拒绝移动。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use ym_model::{tr, Section, SectionLevel};

use crate::{FlowItem, ProjectIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    Before,
    After,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovePlan {
    /// 相对项目根目录
    pub file: PathBuf,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum MoveError {
    #[error("{}", tr!("找不到章节：{id}", "Section not found: {id}", id = .0))]
    NotFound(String),
    #[error("{}", tr!("只能在同级章节之间移动", "Sections can only be moved among sections of the same level"))]
    LevelMismatch,
    #[error("{}", tr!("不能移动到自身内部", "A section cannot be moved into itself"))]
    IntoItself,
    #[error("{}", tr!(
        "这两节分布在不同的文件中，暂不支持跨文件移动",
        "These sections are in different files; moving across files is not supported yet"
    ))]
    CrossFile,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// 文档顺序中的一个位置：文件与字节偏移
#[derive(Debug, Clone, PartialEq, Eq)]
struct Point {
    file: PathBuf,
    offset: usize,
}

#[derive(Debug)]
enum Event {
    Heading { at: Point, level: SectionLevel },
    Break { at: Point },
}

struct Doc<'a> {
    index: &'a ProjectIndex,
    texts: HashMap<PathBuf, String>,
    events: Vec<Event>,
}

impl<'a> Doc<'a> {
    fn load(index: &'a ProjectIndex) -> Result<Self, MoveError> {
        let mut texts = HashMap::new();
        for f in &index.files {
            texts.insert(
                f.path.clone(),
                fs::read_to_string(index.root.join(&f.path))?,
            );
        }
        // (父文件, \input 起点) → 被引入的文件
        let children: HashMap<(PathBuf, usize), PathBuf> = index
            .included_from
            .iter()
            .map(|(child, site)| ((site.file.clone(), site.start), child.clone()))
            .collect();
        let mut doc = Doc {
            index,
            texts,
            events: Vec::new(),
        };
        let mut visited = HashSet::new();
        doc.walk(&index.main, &children, &mut visited);
        Ok(doc)
    }

    /// 与建索引时相同的顺序展开全文，记录标题与终止位置
    fn walk(
        &mut self,
        path: &Path,
        children: &HashMap<(PathBuf, usize), PathBuf>,
        visited: &mut HashSet<PathBuf>,
    ) {
        if !visited.insert(path.to_path_buf()) {
            return;
        }
        let Some(file) = self.index.files.iter().find(|f| f.path == path) else {
            return;
        };
        let mut breaks = file.breaks.iter().copied().peekable();
        let at = |offset| Point {
            file: path.to_path_buf(),
            offset,
        };
        for item in &file.flow {
            let offset = match *item {
                FlowItem::Heading(i) => file.headings[i].span.start,
                FlowItem::Include(i) => file.includes[i].span.start,
                _ => continue,
            };
            while let Some(b) = breaks.next_if(|&b| b < offset) {
                self.events.push(Event::Break { at: at(b) });
            }
            match *item {
                FlowItem::Heading(i) => self.events.push(Event::Heading {
                    at: at(offset),
                    level: file.headings[i].level,
                }),
                FlowItem::Include(_) => {
                    if let Some(child) = children.get(&(path.to_path_buf(), offset)) {
                        let child = child.clone();
                        self.walk(&child, children, visited);
                    }
                }
                _ => {}
            }
        }
        for b in breaks {
            self.events.push(Event::Break { at: at(b) });
        }
    }

    fn text(&self, file: &Path) -> &str {
        self.texts.get(file).map(String::as_str).unwrap_or("")
    }

    /// 文件中 `offset` 之前只有空白与注释行
    fn blank_before(&self, p: &Point) -> bool {
        self.text(&p.file)[..p.offset].lines().all(|l| {
            let t = l.trim();
            t.is_empty() || t.starts_with('%')
        })
    }

    /// 包含链：从主文件到 `file`
    fn chain(&self, file: &Path) -> Vec<PathBuf> {
        let mut out = vec![file.to_path_buf()];
        let mut cur = file.to_path_buf();
        while let Some(site) = self.index.included_from.get(&cur) {
            cur = site.file.clone();
            if out.contains(&cur) {
                break;
            }
            out.push(cur.clone());
        }
        out.reverse();
        out
    }

    /// 文件中 `offset` 之后只有空白与注释
    fn blank_after(&self, p: &Point) -> bool {
        self.text(&p.file)[p.offset..].lines().all(|l| {
            let t = l.trim();
            t.is_empty() || t.starts_with('%')
        })
    }

    /// 标题之后、文档顺序中第一个同级或更高级的标题或终止位置；没有则为 None（到文档末尾）
    fn boundary(&self, heading: &Point, level: SectionLevel) -> Result<Option<Point>, MoveError> {
        let i = self
            .events
            .iter()
            .position(|e| matches!(e, Event::Heading { at, .. } if at == heading))
            .ok_or(MoveError::CrossFile)?;
        Ok(self.events[i + 1..].iter().find_map(|e| match e {
            Event::Heading { at, level: l } if *l <= level => Some(at.clone()),
            Event::Break { at } => Some(at.clone()),
            _ => None,
        }))
    }

    /// 范围终点在 `file` 中的位置：终点在 `file` 里就直接用；在 `file` 引入的子文件里，
    /// 就提升到引入它的 `\input`（要求子文件中终点之前只有空白）；在 `file` 之外则到 `file` 末尾。
    fn end_in(&self, file: &Path, boundary: &Option<Point>) -> Option<usize> {
        let eof = self.text(file).len();
        let Some(b) = boundary else {
            return Some(eof);
        };
        if !self.chain(&b.file).iter().any(|f| f == file) {
            return Some(eof);
        }
        let mut p = b.clone();
        while p.file != file {
            if !self.blank_before(&p) {
                return None;
            }
            let site = self.index.included_from.get(&p.file)?;
            p = Point {
                file: site.file.clone(),
                offset: site.start,
            };
        }
        Some(p.offset)
    }

    /// 插入位置在各层文件中的候选：之前插入时只要求前面是空白，之后插入时只要求后面是空白，
    /// 满足就提升到父文件里那条 `\input` 的前面或后面
    fn insertion_points(
        &self,
        sec: &Section,
        position: Position,
    ) -> Result<Vec<(PathBuf, usize)>, MoveError> {
        let heading = Point {
            file: sec.span.file.clone(),
            offset: sec.span.start,
        };
        let mut p = match position {
            Position::Before => heading,
            Position::After => {
                let boundary = self.boundary(&heading, sec.level)?;
                let Some(end) = self.end_in(&heading.file, &boundary) else {
                    return Ok(Vec::new());
                };
                Point {
                    file: heading.file,
                    offset: end,
                }
            }
        };
        let mut out = vec![(p.file.clone(), p.offset)];
        loop {
            let blank = match position {
                Position::Before => self.blank_before(&p),
                Position::After => self.blank_after(&p),
            };
            let site = match self.index.included_from.get(&p.file) {
                Some(site) if blank => site,
                _ => break,
            };
            let offset = match position {
                Position::Before => site.start,
                Position::After => {
                    // \input 所在行的下一行开头（该行后面只有空白时）
                    let text = self.text(&site.file);
                    let rest = &text[site.end..];
                    match rest.find('\n') {
                        Some(i) if rest[..i].trim().is_empty() => site.end + i + 1,
                        None if rest.trim().is_empty() => text.len(),
                        _ => site.end,
                    }
                }
            };
            p = Point {
                file: site.file.clone(),
                offset,
            };
            out.push((p.file.clone(), p.offset));
        }
        Ok(out)
    }

    /// 章节在各层文件中的范围：先是标题所在文件，若占满该文件就依次提升到父文件
    fn ranges(&self, sec: &Section) -> Result<Vec<(PathBuf, usize, usize)>, MoveError> {
        let heading = Point {
            file: sec.span.file.clone(),
            offset: sec.span.start,
        };
        let boundary = self.boundary(&heading, sec.level)?;
        let mut out = Vec::new();
        let mut start = heading;
        while let Some(end) = self.end_in(&start.file, &boundary) {
            out.push((start.file.clone(), start.offset, end));
            let end = Point {
                file: start.file.clone(),
                offset: end,
            };
            if !(self.blank_before(&start) && self.blank_after(&end)) {
                break;
            }
            let Some(site) = self.index.included_from.get(&start.file) else {
                break;
            };
            start = Point {
                file: site.file.clone(),
                offset: site.start,
            };
        }
        Ok(out)
    }
}

fn find<'s>(sections: &'s [Section], id: &str) -> Option<&'s Section> {
    sections
        .iter()
        .find_map(|s| (s.id == id).then_some(s).or_else(|| find(&s.children, id)))
}

fn contains(section: &Section, id: &str) -> bool {
    section.id == id || section.children.iter().any(|c| contains(c, id))
}

/// 退到行首（该行在 offset 之前只有空白时），保证整行移动
fn line_start(text: &str, offset: usize) -> usize {
    let ls = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    if text[ls..offset].trim().is_empty() {
        ls
    } else {
        offset
    }
}

/// 计算把章节 `id` 移到章节 `target` 之前或之后的结果。
pub fn plan_move(
    index: &ProjectIndex,
    id: &str,
    target: &str,
    position: Position,
) -> Result<MovePlan, MoveError> {
    let s = find(&index.outline, id).ok_or_else(|| MoveError::NotFound(id.to_owned()))?;
    let t = find(&index.outline, target).ok_or_else(|| MoveError::NotFound(target.to_owned()))?;
    if s.level != t.level {
        return Err(MoveError::LevelMismatch);
    }
    if contains(s, target) {
        return Err(MoveError::IntoItself);
    }
    let doc = Doc::load(index)?;
    let moving = doc.ranges(s)?;
    let targets = doc.insertion_points(t, position)?;
    // 选最深的共同文件
    let (file, a, b, p) = moving
        .iter()
        .find_map(|(f, a, b)| {
            targets
                .iter()
                .find(|(tf, _)| tf == f)
                .map(|(_, p)| (f.clone(), *a, *b, *p))
        })
        .ok_or(MoveError::CrossFile)?;
    let from = Point { file, offset: a };
    let to = Point {
        file: from.file.clone(),
        offset: b,
    };
    let at = Point {
        file: from.file.clone(),
        offset: p,
    };
    let old = doc.text(&from.file);
    let a = line_start(old, from.offset);
    let b = line_start(old, to.offset);
    let p = line_start(old, at.offset);
    if (a..=b).contains(&p) {
        // 原地不动
        return Ok(MovePlan {
            file: from.file.clone(),
            old_text: old.to_owned(),
            new_text: old.to_owned(),
        });
    }
    let mut block = old[a..b].to_owned();
    if !block.ends_with('\n') {
        block.push('\n');
    }
    let mut rest = format!("{}{}", &old[..a], &old[b..]);
    let p = if p > b { p - (b - a) } else { p };
    // 插入到文件末尾且末尾没有换行时，先补一个
    let p = if p >= rest.len() {
        if !rest.is_empty() && !rest.ends_with('\n') {
            rest.push('\n');
        }
        rest.len()
    } else {
        p
    };
    rest.insert_str(p, &block);
    Ok(MovePlan {
        file: from.file.clone(),
        old_text: old.to_owned(),
        new_text: rest,
    })
}
