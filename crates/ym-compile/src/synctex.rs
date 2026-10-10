//! SyncTeX 正向（源码 → PDF）与反向（PDF → 源码）查询。
//!
//! 坐标统一为 PDF 点（bp），原点在页面左上角，与 PDF.js 视口坐标一致。

use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// TeX 的 sp 与 PDF 点（bp）之比：65536 × 72.27 / 72。
const SP_PER_BP: f64 = 65781.76;

#[derive(Debug, Clone, Copy)]
struct Record {
    page: u32,
    tag: u32,
    line: u32,
    /// `(` 水平盒、`[` 垂直盒，其余（x k g $ h v）为点状记录
    kind: u8,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    d: f64,
}

/// PDF 上的一个矩形区域。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PdfRect {
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceLocation {
    /// 相对项目根目录
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Default)]
pub struct SyncTex {
    /// tag → 相对项目根目录的路径；不在项目内的文件（宏包等）不记录
    inputs: HashMap<u32, String>,
    records: Vec<Record>,
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

impl SyncTex {
    /// 读取 `.synctex.gz`（或未压缩的 `.synctex`）。
    pub fn load(path: &Path, root: &Path) -> std::io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let mut text = String::new();
        if bytes.starts_with(&[0x1f, 0x8b]) {
            flate2::read::GzDecoder::new(&bytes[..]).read_to_string(&mut text)?;
        } else {
            text = String::from_utf8_lossy(&bytes).into_owned();
        }
        Ok(Self::parse(&text, root))
    }

    pub fn parse(text: &str, root: &Path) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let mut st = SyncTex::default();
        let mut unit = 1.0;
        let mut mag = 1.0;
        let (mut x_off, mut y_off) = (0.0, 0.0);
        let mut page = 0;
        let mut in_content = false;

        for line in text.lines() {
            if !in_content {
                if let Some(rest) = line.strip_prefix("Input:") {
                    st.add_input(rest, &root);
                } else if let Some(v) = line.strip_prefix("Unit:") {
                    unit = v.trim().parse().unwrap_or(1.0);
                } else if let Some(v) = line.strip_prefix("Magnification:") {
                    mag = v.trim().parse::<f64>().unwrap_or(1000.0) / 1000.0;
                } else if let Some(v) = line.strip_prefix("X Offset:") {
                    x_off = v.trim().parse().unwrap_or(0.0);
                } else if let Some(v) = line.strip_prefix("Y Offset:") {
                    y_off = v.trim().parse().unwrap_or(0.0);
                } else if line == "Content:" {
                    in_content = true;
                }
                continue;
            }
            let Some(&kind) = line.as_bytes().first() else {
                continue;
            };
            match kind {
                b'{' => page = line[1..].parse().unwrap_or(page),
                b'(' | b'[' | b'h' | b'v' | b'x' | b'k' | b'g' | b'$' => {
                    let scale = unit * mag / SP_PER_BP;
                    if let Some(mut r) = parse_record(kind, &line[1..], page, scale) {
                        r.x += x_off * scale;
                        r.y += y_off * scale;
                        st.records.push(r);
                    }
                }
                // 页面中途也可能出现新的 Input
                b'I' => {
                    if let Some(rest) = line.strip_prefix("Input:") {
                        st.add_input(rest, &root);
                    }
                }
                _ => {}
            }
        }
        st
    }

    fn add_input(&mut self, rest: &str, root: &Path) {
        let Some((tag, path)) = rest.split_once(':') else {
            return;
        };
        let (Ok(tag), false) = (tag.parse(), path.is_empty()) else {
            return;
        };
        let abs = normalize(Path::new(path));
        let abs = abs.canonicalize().unwrap_or(abs);
        if let Ok(rel) = abs.strip_prefix(root) {
            self.inputs
                .insert(tag, rel.to_string_lossy().replace('\\', "/"));
        }
    }

    /// 源码第 `line` 行在 PDF 中的位置。该行没有排版记录时取之前最近的一行。
    ///
    /// TeX 在段落结束时才输出整行的盒子，它们常被记到下一条命令所在的行上，
    /// 只有字距、空隙等小记录带着正确的行号，所以这里以点状记录为准：
    /// 取该行记录最多的那条基线，框出它在页面上的横向范围。
    pub fn forward(&self, file: &str, line: u32) -> Option<PdfRect> {
        let recs: Vec<&Record> = self
            .records
            .iter()
            .filter(|r| self.inputs.get(&r.tag).is_some_and(|f| f == file))
            .collect();
        let target = recs
            .iter()
            .map(|r| r.line)
            .filter(|&l| l <= line)
            .max()
            .or_else(|| recs.iter().map(|r| r.line).filter(|&l| l > line).min())?;
        let hits: Vec<&Record> = recs.into_iter().filter(|r| r.line == target).collect();
        let page = hits.iter().map(|r| r.page).min()?;
        let on_page: Vec<&Record> = hits.into_iter().filter(|r| r.page == page).collect();
        let points: Vec<&Record> = on_page
            .iter()
            .copied()
            .filter(|r| r.kind != b'(' && r.kind != b'[')
            .collect();
        let pool = if points.is_empty() { on_page } else { points };

        // 记录最多的基线
        let mut baselines: Vec<(f64, usize)> = Vec::new();
        for r in &pool {
            match baselines.iter_mut().find(|(y, _)| (y - r.y).abs() < 0.5) {
                Some((_, n)) => *n += 1,
                None => baselines.push((r.y, 1)),
            }
        }
        let (y, _) = baselines
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.total_cmp(&a.0)))?;
        let row: Vec<&&Record> = pool.iter().filter(|r| (r.y - y).abs() < 0.5).collect();
        let left = row.iter().map(|r| r.x).fold(f64::MAX, f64::min);
        // 字距与空隙记录的宽度多是行尾填充，不计入
        let right = row
            .iter()
            .map(|r| {
                r.x + if matches!(r.kind, b'k' | b'g') {
                    0.0
                } else {
                    r.w
                }
            })
            .fold(f64::MIN, f64::max);
        let ascent = row.iter().map(|r| r.h).fold(9.0, f64::max);
        let descent = row.iter().map(|r| r.d).fold(3.0, f64::max);
        Some(PdfRect {
            page,
            x: left,
            y: y - ascent,
            w: (right - left).max(8.0),
            h: ascent + descent,
        })
    }

    /// PDF 第 `page` 页上点 (x, y) 对应的源码位置。
    ///
    /// 先用包含该点的最小水平盒确定是哪一行文字（基线），再取这条基线上
    /// 点击位置左侧最近的记录——它带着正确的文件与行号。
    pub fn inverse(&self, page: u32, x: f64, y: f64) -> Option<SourceLocation> {
        let on_page: Vec<&Record> = self.records.iter().filter(|r| r.page == page).collect();
        let known = |r: &&&Record| self.inputs.contains_key(&r.tag);
        let line_box = on_page
            .iter()
            .filter(|r| r.kind == b'(' && r.w > 0.0)
            .filter(|r| {
                x >= r.x - 2.0
                    && x <= r.x + r.w + 2.0
                    && y >= r.y - r.h.max(9.0)
                    && y <= r.y + r.d.max(3.0)
            })
            .min_by(|a, b| (a.w * (a.h + a.d)).total_cmp(&(b.w * (b.h + b.d))));
        let best = match line_box {
            Some(b) => {
                let row: Vec<&&Record> = on_page
                    .iter()
                    .filter(known)
                    .filter(|r| (r.y - b.y).abs() < 0.5)
                    .collect();
                row.iter()
                    .filter(|r| r.x <= x)
                    .max_by(|a, b| a.x.total_cmp(&b.x))
                    .or_else(|| row.iter().min_by(|a, b| a.x.total_cmp(&b.x)))
                    .map(|r| **r)
            }
            None => None,
        }
        .or_else(|| {
            on_page
                .iter()
                .filter(known)
                .min_by(|a, b| dist(a, x, y).total_cmp(&dist(b, x, y)))
                .copied()
        })?;
        Some(SourceLocation {
            file: self.inputs[&best.tag].clone(),
            line: best.line,
        })
    }
}

fn dist(r: &Record, x: f64, y: f64) -> f64 {
    let dx = if x < r.x {
        r.x - x
    } else if x > r.x + r.w {
        x - r.x - r.w
    } else {
        0.0
    };
    let dy = r.y - y;
    dx * dx + dy * dy
}

/// 解析 `tag,line[,col]:x,y[:W,H,D]`。
fn parse_record(kind: u8, rest: &str, page: u32, scale: f64) -> Option<Record> {
    let mut parts = rest.split(':');
    let mut link = parts.next()?.split(',');
    let tag = link.next()?.parse().ok()?;
    let line = link.next()?.parse().ok()?;
    let mut pos = parts.next()?.split(',');
    let x: f64 = pos.next()?.parse().ok()?;
    let y: f64 = pos.next()?.parse().ok()?;
    let mut size = parts
        .next()
        .unwrap_or("")
        .split(',')
        .map(|v| v.parse::<f64>().unwrap_or(0.0));
    let (w, h, d) = (
        size.next().unwrap_or(0.0),
        size.next().unwrap_or(0.0),
        size.next().unwrap_or(0.0),
    );
    Some(Record {
        page,
        tag,
        line,
        kind,
        x: x * scale,
        y: y * scale,
        w: w * scale,
        h: h * scale,
        d: d * scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(root: &str) -> String {
        format!(
            "SyncTeX Version:1\nInput:1:{root}/main.tex\nInput:2:\nInput:43:{root}/./sections/intro.tex\nOutput:pdf\nMagnification:1000\nUnit:1\nX Offset:0\nY Offset:0\nContent:\n{{1\n[1,9:4736287,46220575:26673152,41484288,0\n(43,1:8799519,12176856:22609920,806155,87818\nk43,1:18346627,12176856:9547108\n)\n(43,2:8799519,14000000:22609920,650000,120000\ng43,2:9000000,14000000\n)\n(43,4:8799519,16000000:10000000,650000,120000\n)\n]\n}}1\n"
        )
    }

    #[test]
    fn forward_and_inverse() {
        let root = "/proj";
        let st = SyncTex::parse(&sample(root), Path::new(root));
        assert_eq!(
            st.inputs.get(&43).map(String::as_str),
            Some("sections/intro.tex")
        );
        assert!(!st.inputs.contains_key(&2));

        let r = st.forward("sections/intro.tex", 2).unwrap();
        assert_eq!(r.page, 1);
        // 第 2 行只有一个 glue 点：框从该点开始，宽度取最小值
        assert!((r.x - 9000000.0 / SP_PER_BP).abs() < 1e-6);
        assert!((r.y - (14000000.0 / SP_PER_BP - 9.0)).abs() < 1e-6);

        // 第 3 行没有记录：落到之前最近的第 2 行
        assert_eq!(st.forward("sections/intro.tex", 3).unwrap(), r);

        // 点在第 2 行的盒子里、glue 点右侧
        let hit = st
            .inverse(1, r.x + 20.0, 14000000.0 / SP_PER_BP - 2.0)
            .unwrap();
        assert_eq!(
            hit,
            SourceLocation {
                file: "sections/intro.tex".into(),
                line: 2
            }
        );
        assert_eq!(st.forward("other.tex", 1), None);
    }
}
