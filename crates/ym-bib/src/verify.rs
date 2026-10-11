//! 文献核查：有 DOI 的条目经 doi.org 解析（各注册机构通用），没有的到 Crossref 按标题查找，arXiv 条目查 arXiv；，确认文献真实存在、标题作者年份与登记信息一致，
//! 缺 DOI 时给出建议。比较逻辑是纯函数（[`compare`]），联网部分在 [`verify`]。
//!
//! 只发送标题、第一作者、年份、DOI 等公开书目信息。

use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

use ym_model::{tr, BibEntry};

/// 外部登记的书目信息
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Found {
    pub title: String,
    /// 作者姓（family name），按顺序
    pub authors: Vec<String>,
    /// 第一作者的名（given name）。中文姓名常被登记反，比较时一并考虑
    pub first_given: Option<String>,
    pub year: Option<i32>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    /// crossref / arxiv
    pub source: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// 找到且信息一致
    Ok,
    /// 找到，但标题、作者或年份明显不一致
    Mismatch,
    /// DOI 有效，但登记机构不提供可核对的书目信息
    Resolved,
    /// 没有找到对应的登记记录（可能是书、报告、网页，也可能有误或不存在）
    NotFound,
    /// 条目缺少标题，无法核查
    Skipped,
    /// 网络等错误
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Verification {
    pub key: String,
    pub status: Status,
    /// 给用户看的问题与提示（当前界面语言）
    pub notes: Vec<String>,
    pub found: Option<Found>,
    /// 条目没有 DOI、且找到了可信的匹配时建议补上的 DOI
    pub suggested_doi: Option<String>,
}

/// 去掉 LaTeX 命令与花括号，转小写，只保留字母、数字和汉字，用于比较标题
pub fn normalize(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            // 跳过命令名（\emph、\textit），保留其参数文字；\& 等转义保留字符本身
            if chars.peek().is_some_and(|n| n.is_ascii_alphabetic()) {
                while chars.peek().is_some_and(|n| n.is_ascii_alphabetic()) {
                    chars.next();
                }
            }
            continue;
        }
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }
    }
    out.trim_end().to_owned()
}

/// 标题相似度：字符二元组的 Dice 系数（对中英文都适用），1 为完全相同
pub fn similarity(a: &str, b: &str) -> f64 {
    let grams = |s: &str| {
        let cs: Vec<char> = normalize(s).chars().filter(|c| *c != ' ').collect();
        cs.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>()
    };
    let (ga, mut gb) = (grams(a), grams(b));
    if ga.is_empty() || gb.is_empty() {
        return if normalize(a) == normalize(b) {
            1.0
        } else {
            0.0
        };
    }
    let total = ga.len() + gb.len();
    let mut common = 0;
    for g in &ga {
        if let Some(i) = gb.iter().position(|x| x == g) {
            gb.swap_remove(i);
            common += 1;
        }
    }
    2.0 * common as f64 / total as f64
}

/// 第一作者的姓：`Vaswani, Ashish and others` → vaswani；`Ashish Vaswani` → vaswani
pub fn first_author_family(author: &str) -> Option<String> {
    let first = author.split(" and ").next()?.trim();
    let family = match first.split_once(',') {
        Some((f, _)) => f,
        None => first.rsplit(' ').next().unwrap_or(first),
    };
    let n = fold(family);
    (!n.is_empty() && n != "others").then_some(n)
}

/// 去掉重音与 LaTeX 转义，只保留小写字母（Müller、M{\"u}ller → muller）
fn fold(s: &str) -> String {
    normalize(s)
        .chars()
        .filter_map(|c| match c {
            'à'..='å' => Some('a'),
            'è'..='ë' => Some('e'),
            'ì'..='ï' => Some('i'),
            'ò'..='ö' | 'ø' => Some('o'),
            'ù'..='ü' => Some('u'),
            'ñ' => Some('n'),
            'ç' => Some('c'),
            'ý' | 'ÿ' => Some('y'),
            'ı' | 'ī' => Some('i'),
            'ş' | 'ś' | 'š' => Some('s'),
            'ğ' => Some('g'),
            'ł' => Some('l'),
            'ć' | 'č' => Some('c'),
            'ž' | 'ź' | 'ż' => Some('z'),
            'ő' | 'ō' => Some('o'),
            'ű' | 'ū' => Some('u'),
            'ā' => Some('a'),
            'ě' | 'ē' => Some('e'),
            'ř' => Some('r'),
            'ň' | 'ń' => Some('n'),
            'ß' => Some('s'),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// 姓氏是否相同：包含关系或高度相似（容忍转写差异）
fn same_family(a: &str, b: &str) -> bool {
    b.is_empty() || a.contains(b) || b.contains(a) || similarity(a, b) >= 0.75
}

fn entry_year(e: &BibEntry) -> Option<i32> {
    let y = e.field("year").or_else(|| e.field("date"))?;
    y.trim_matches(|c: char| !c.is_ascii_digit())
        .get(..4)?
        .parse()
        .ok()
}

/// 把条目与外部记录比较，得出状态与说明
pub fn compare(e: &BibEntry, f: &Found) -> (Status, Vec<String>) {
    let mut notes = Vec::new();
    let mut serious = false;
    let title = e.field("title").unwrap_or("");
    let sim = similarity(title, &f.title);
    if sim < 0.85 {
        serious = sim < 0.6;
        notes.push(tr!(
            "标题与登记信息不同：{t}",
            "Title differs from the registered record: {t}",
            t = f.title
        ));
    }
    if let (Some(a), Some(b)) = (
        e.field("author").and_then(first_author_family),
        f.authors.first().map(|x| fold(x)),
    ) {
        let given = f.first_given.as_deref().map(fold).unwrap_or_default();
        if !same_family(&a, &b) && !(given.len() >= 2 && given.contains(&a)) {
            serious = true;
            notes.push(tr!(
                "第一作者不一致：登记为 {b}",
                "First author differs: registered as {b}",
                b = f.authors[0]
            ));
        }
    }
    if let (Some(a), Some(b)) = (entry_year(e), f.year) {
        if a != b {
            // 网络优先发表与正式卷期常差一年，只作提示
            if (a - b).abs() > 1 {
                serious = true;
            }
            notes.push(tr!(
                "年份不一致：条目为 {a}，登记为 {b}",
                "Year differs: {a} in the entry, {b} registered"
            ));
        }
    }
    if let (Some(d), Some(fd)) = (e.field("doi"), &f.doi) {
        if !d.trim().eq_ignore_ascii_case(fd) {
            notes.push(tr!(
                "DOI 与登记的不同：{fd}",
                "DOI differs from the record: {fd}"
            ));
        }
    }
    (
        if serious {
            Status::Mismatch
        } else {
            Status::Ok
        },
        notes,
    )
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!(
            "YanMo/",
            env!("CARGO_PKG_VERSION"),
            " (https://yanmo.ink; citation check)"
        ))
        .build()
}

fn year_of(item: &Value) -> Option<i32> {
    ["issued", "published-print", "published-online", "created"]
        .iter()
        .find_map(|k| item[k]["date-parts"][0][0].as_i64())
        .map(|y| y as i32)
}

/// 登记信息常把副标题单独存放：XGBoost + A Scalable Tree Boosting System
fn with_subtitle(title: &str, subtitle: Option<&str>) -> String {
    match subtitle.filter(|s| !s.is_empty() && !title.contains(s)) {
        Some(sub) => format!("{title}: {sub}"),
        None => title.to_owned(),
    }
}

/// 解析 Crossref 的 work 记录
pub fn parse_crossref(item: &Value) -> Found {
    Found {
        title: with_subtitle(
            item["title"][0].as_str().unwrap_or(""),
            item["subtitle"][0].as_str(),
        ),
        authors: item["author"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["family"].as_str().or(x["name"].as_str()))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        first_given: item["author"][0]["given"].as_str().map(str::to_owned),
        year: year_of(item),
        venue: item["container-title"][0].as_str().map(str::to_owned),
        doi: item["DOI"].as_str().map(str::to_lowercase),
        source: "crossref",
    }
}

/// 解析 doi.org 内容协商返回的 CSL JSON（Crossref、DataCite、mEDRA 等注册机构通用）
pub fn parse_csl(item: &Value) -> Found {
    let text = |v: &Value| match v {
        Value::Array(a) => a.first().and_then(Value::as_str).map(str::to_owned),
        Value::String(s) => Some(s.clone()),
        _ => None,
    };
    Found {
        title: with_subtitle(
            &text(&item["title"]).unwrap_or_default(),
            text(&item["subtitle"]).as_deref(),
        ),
        authors: item["author"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x["family"].as_str().or(x["literal"].as_str()))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        first_given: item["author"][0]["given"].as_str().map(str::to_owned),
        year: year_of(item),
        venue: text(&item["container-title"]).or_else(|| text(&item["publisher"])),
        doi: item["DOI"].as_str().map(str::to_lowercase),
        source: "doi.org",
    }
}

/// 遇到限流（429）时按 1、2、4 秒退避重试
// ureq::Error 本身较大，原样传递即可
#[allow(clippy::result_large_err)]
fn call(req: ureq::Request) -> Result<ureq::Response, ureq::Error> {
    let mut wait = 1;
    loop {
        match req.clone().call() {
            Err(ureq::Error::Status(429, r)) if wait <= 4 => {
                let secs = r
                    .header("retry-after")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(wait)
                    .min(10);
                std::thread::sleep(Duration::from_secs(secs));
                wait *= 2;
            }
            other => return other,
        }
    }
}

enum DoiLookup {
    Found(Found),
    /// DOI 存在，但没有可用的书目信息
    Exists,
    Missing,
}

fn lookup_doi(doi: &str) -> Result<DoiLookup, String> {
    let req = agent()
        .get(&format!("https://doi.org/{doi}"))
        .set("Accept", "application/vnd.citationstyles.csl+json");
    match call(req) {
        Ok(r) => {
            // 部分注册机构不支持内容协商，会跳到 HTML 落地页
            if let Ok(v) = r.into_json::<Value>() {
                let f = parse_csl(&v);
                if !f.title.is_empty() {
                    return Ok(DoiLookup::Found(f));
                }
            }
        }
        Err(ureq::Error::Status(404 | 406, _)) => {}
        Err(e) => return Err(e.to_string()),
    }
    // 直接查 DOI 系统的句柄记录：responseCode 1 表示存在
    let v: Value = match call(agent().get(&format!("https://doi.org/api/handles/{doi}"))) {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(404, _)) => return Ok(DoiLookup::Missing),
        Err(e) => return Err(e.to_string()),
    };
    Ok(if v["responseCode"] == 1 {
        DoiLookup::Exists
    } else {
        DoiLookup::Missing
    })
}

fn crossref_search(e: &BibEntry) -> Result<Option<Found>, String> {
    let title = e.field("title").unwrap_or("");
    let author = e.field("author").and_then(first_author_family);
    let year = entry_year(e);
    let mut query = normalize(title);
    for extra in [author.clone(), year.map(|y| y.to_string())]
        .into_iter()
        .flatten()
    {
        query.push(' ');
        query.push_str(&extra);
    }
    let v: Value = call(
        agent()
            .get("https://api.crossref.org/works")
            .query("query.bibliographic", &query)
            .query("rows", "10")
            .query(
                "select",
                "DOI,title,author,issued,published-print,published-online,container-title",
            ),
    )
    .map_err(|e| e.to_string())?
    .into_json()
    .map_err(|e| e.to_string())?;
    // 标题相近的候选里，作者、年份也吻合的优先（同名论文常有转载、预印本等多条记录）
    let score = |f: &Found| {
        let mut s = similarity(title, &f.title);
        if let (Some(a), Some(b)) = (&author, f.authors.first().map(|x| fold(x))) {
            if same_family(a, &b) {
                s += 0.2;
            }
        }
        if let (Some(a), Some(b)) = (year, f.year) {
            s += match (a - b).abs() {
                0 => 0.2,
                1 => 0.1,
                _ => 0.0,
            };
        }
        s
    };
    let best = v["message"]["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(parse_crossref)
        .filter(|f| similarity(title, &f.title) >= 0.75)
        .max_by(|a, b| score(a).total_cmp(&score(b)));
    Ok(best)
}

fn arxiv_id(e: &BibEntry) -> Option<String> {
    let is_arxiv = e
        .field("archiveprefix")
        .is_some_and(|p| p.eq_ignore_ascii_case("arxiv"));
    if is_arxiv {
        if let Some(id) = e.field("eprint") {
            return Some(id.trim().to_owned());
        }
    }
    for field in ["url", "journal", "note", "eprint"] {
        if let Some(crate::import::Identifier::Arxiv(id)) =
            e.field(field).and_then(crate::import::detect)
        {
            return Some(id);
        }
    }
    None
}

/// 核查一个条目（联网）
pub fn verify(e: &BibEntry) -> Verification {
    let mut v = Verification {
        key: e.key.clone(),
        status: Status::Skipped,
        notes: Vec::new(),
        found: None,
        suggested_doi: None,
    };
    if e.field("title").is_none_or(|t| t.trim().is_empty()) {
        v.notes.push(tr!(
            "条目没有标题，无法核查",
            "The entry has no title to check"
        ));
        return v;
    }
    let doi = e.field("doi").map(|d| {
        d.trim()
            .trim_start_matches("https://doi.org/")
            .to_lowercase()
    });
    let looked_up = if let Some(d) = &doi {
        match lookup_doi(d) {
            Ok(DoiLookup::Found(f)) => Ok(Some(f)),
            Ok(DoiLookup::Missing) => Ok(None),
            Ok(DoiLookup::Exists) => {
                v.status = Status::Resolved;
                v.notes.push(tr!(
                    "DOI 有效，但其登记机构不提供可核对的书目信息",
                    "The DOI is valid, but its registration agency provides no metadata to check against"
                ));
                return v;
            }
            Err(e) => Err(e),
        }
    } else if let Some(id) = arxiv_id(e) {
        crate::import::fetch(&crate::import::Identifier::Arxiv(id.clone()))
            .map(|a| {
                Some(Found {
                    title: a.field("title").unwrap_or("").to_owned(),
                    authors: a
                        .field("author")
                        .map(|s| {
                            s.split(" and ")
                                .map(|x| x.split(',').next().unwrap_or(x).trim().to_owned())
                                .collect()
                        })
                        .unwrap_or_default(),
                    first_given: None,
                    year: entry_year(&a),
                    venue: Some("arXiv".into()),
                    doi: a.field("doi").map(str::to_lowercase),
                    source: "arxiv",
                })
            })
            .map_err(|e| e.to_string())
    } else {
        crossref_search(e)
    };
    match looked_up {
        Err(err) => {
            v.status = Status::Error;
            v.notes.push(tr!("查询失败：{err}", "Lookup failed: {err}"));
        }
        Ok(None) => {
            v.status = Status::NotFound;
            v.notes.push(if doi.is_some() {
                tr!(
                    "DOI 无法解析，请确认 DOI 是否正确",
                    "The DOI does not resolve; check that it is correct"
                )
            } else {
                tr!(
                    "未在 Crossref 找到对应记录（书籍、报告、网页常见此情况），请人工确认",
                    "No matching Crossref record (common for books, reports and web pages); please check it by hand"
                )
            });
        }
        Ok(Some(found)) => {
            let (status, notes) = compare(e, &found);
            // 按标题搜索到的记录若对不上，多半是找错了（同名文献、转载），不能据此说条目有误
            if doi.is_none() && found.source == "crossref" && status != Status::Ok {
                v.status = Status::NotFound;
                v.notes.push(tr!(
                    "未找到可靠的匹配；最接近的记录：{t}（{y}）",
                    "No reliable match; closest record: {t} ({y})",
                    t = found.title,
                    y = found.year.map(|y| y.to_string()).unwrap_or_default()
                ));
                v.found = Some(found);
                return v;
            }
            v.status = status;
            v.notes = notes;
            if doi.is_none() && status == Status::Ok {
                v.suggested_doi = found.doi.clone();
                if let Some(d) = &found.doi {
                    v.notes.push(tr!("可补充 DOI：{d}", "Suggested DOI: {d}"));
                }
            }
            v.found = Some(found);
        }
    }
    v
}

/// 并发核查多个条目（最多 2 个同时进行，对 Crossref 友好），结果按输入顺序返回
pub fn verify_all(entries: &[BibEntry]) -> Vec<Verification> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<Verification>>> = Mutex::new(vec![None; entries.len()]);
    std::thread::scope(|s| {
        for _ in 0..2.min(entries.len()) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some(e) = entries.get(i) else { break };
                let r = verify(e);
                if let Ok(mut o) = out.lock() {
                    o[i] = Some(r);
                }
            });
        }
    });
    out.into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use ym_model::SourceSpan;

    fn entry(fields: &[(&str, &str)]) -> BibEntry {
        BibEntry {
            key: "k".into(),
            entry_type: "article".into(),
            fields: fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<BTreeMap<_, _>>(),
            span: SourceSpan {
                file: "refs.bib".into(),
                start: 0,
                end: 0,
                line: 1,
            },
            citations: Vec::new(),
        }
    }

    fn found(title: &str, author: &str, year: i32) -> Found {
        Found {
            title: title.into(),
            authors: vec![author.into()],
            year: Some(year),
            source: "crossref",
            ..Default::default()
        }
    }

    #[test]
    fn titles_compare_ignoring_latex_and_case() {
        assert!(
            similarity(
                "{Attention} Is All You \\emph{Need}",
                "Attention is all you need"
            ) > 0.99
        );
        assert!(similarity("Deep residual learning", "Attention is all you need") < 0.4);
        assert!(similarity("耕地破碎化的时空演变", "耕地破碎化时空演变") > 0.8);
    }

    #[test]
    fn first_author_forms() {
        assert_eq!(
            first_author_family("Vaswani, Ashish and others").as_deref(),
            Some("vaswani")
        );
        assert_eq!(
            first_author_family("Ashish Vaswani and Noam Shazeer").as_deref(),
            Some("vaswani")
        );
        assert_eq!(
            first_author_family("M{\\\"u}ller, Hans").as_deref(),
            Some("muller")
        );
    }

    #[test]
    fn matching_entry_is_ok() {
        let e = entry(&[
            ("title", "Attention Is All You Need"),
            ("author", "Vaswani, Ashish and others"),
            ("year", "2017"),
        ]);
        let (s, notes) = compare(&e, &found("Attention is All you Need", "Vaswani", 2017));
        assert_eq!(s, Status::Ok);
        assert!(notes.is_empty());
    }

    #[test]
    fn one_year_off_is_a_note_not_a_mismatch() {
        let e = entry(&[
            ("title", "A study"),
            ("author", "Doe, Jane"),
            ("year", "2021"),
        ]);
        let (s, notes) = compare(&e, &found("A study", "Doe", 2020));
        assert_eq!(s, Status::Ok);
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn wrong_author_or_title_is_a_mismatch() {
        let e = entry(&[
            ("title", "A study of fragmentation"),
            ("author", "Doe, Jane"),
            ("year", "2020"),
        ]);
        assert_eq!(
            compare(&e, &found("A study of fragmentation", "Smith", 2020)).0,
            Status::Mismatch
        );
        assert_eq!(
            compare(&e, &found("Something else entirely", "Doe", 2020)).0,
            Status::Mismatch
        );
    }

    #[test]
    fn parses_crossref_record() {
        let v: Value = serde_json::from_str(r#"{"title":["Deep Residual Learning"],"author":[{"family":"He","given":"Kaiming"}],"issued":{"date-parts":[[2016,6]]},"container-title":["CVPR"],"DOI":"10.1109/CVPR.2016.90"}"#).unwrap();
        let f = parse_crossref(&v);
        assert_eq!(f.authors, vec!["He"]);
        assert_eq!(f.year, Some(2016));
        assert_eq!(f.doi.as_deref(), Some("10.1109/cvpr.2016.90"));
    }

    #[test]
    #[ignore = "需要网络"]
    fn live_lookup() {
        // 有 DOI：直接核对
        let e = entry(&[
            ("title", "Deep Residual Learning for Image Recognition"),
            ("author", "He, Kaiming and others"),
            ("year", "2016"),
            ("doi", "10.1109/CVPR.2016.90"),
        ]);
        let v = verify(&e);
        eprintln!("{v:?}");
        assert_eq!(v.status, Status::Ok);
        // 没有 DOI：按标题找到并建议 DOI
        let e = entry(&[
            ("title", "Deep Residual Learning for Image Recognition"),
            ("author", "He, Kaiming and others"),
            ("year", "2016"),
        ]);
        let v = verify(&e);
        eprintln!("{v:?}");
        assert_eq!(v.status, Status::Ok);
        assert_eq!(v.suggested_doi.as_deref(), Some("10.1109/cvpr.2016.90"));
        // 作者写错
        let e = entry(&[
            ("title", "Deep Residual Learning for Image Recognition"),
            ("author", "Smith, John"),
            ("year", "2016"),
            ("doi", "10.1109/CVPR.2016.90"),
        ]);
        assert_eq!(verify(&e).status, Status::Mismatch);
    }
}

/// 被引文献的摘要，供 AI 判断引用是否支撑论述
#[derive(Debug, Clone, Serialize)]
pub struct CitedWork {
    pub key: String,
    pub title: String,
    /// 简短作者：Vaswani et al.
    pub authors: String,
    pub year: Option<i32>,
    pub r#abstract: Option<String>,
    /// semantic_scholar / crossref
    pub source: Option<&'static str>,
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn s2(path: &str) -> Option<String> {
    let url = format!("https://api.semanticscholar.org/graph/v1/paper/{path}");
    let v: Value = call(agent().get(&url).query("fields", "title,abstract"))
        .ok()?
        .into_json()
        .ok()?;
    // search/match 返回 {data: [...]}
    let item = if v["data"].is_array() {
        &v["data"][0]
    } else {
        &v
    };
    item["abstract"]
        .as_str()
        .map(str::to_owned)
        .filter(|a| !a.trim().is_empty())
}

/// 取条目的摘要：先查 Semantic Scholar（DOI、arXiv 编号或标题），再试 Crossref 登记的摘要
pub fn fetch_abstract(e: &BibEntry) -> CitedWork {
    let title = e.field("title").unwrap_or("").to_owned();
    let authors = e
        .field("author")
        .map(|a| {
            let n = a.split(" and ").count();
            let first = a.split(" and ").next().unwrap_or("");
            let family = first
                .split_once(',')
                .map(|(f, _)| f)
                .unwrap_or_else(|| first.rsplit(' ').next().unwrap_or(first))
                .trim()
                .trim_matches(['{', '}'])
                .to_owned();
            if n > 1 {
                format!("{family} et al.")
            } else {
                family
            }
        })
        .unwrap_or_default();
    let doi = e
        .field("doi")
        .map(|d| d.trim().trim_start_matches("https://doi.org/").to_owned());
    let mut found = None;
    if let Some(d) = &doi {
        found = s2(&format!("DOI:{d}")).map(|a| (a, "semantic_scholar"));
    }
    if found.is_none() {
        if let Some(id) = arxiv_id(e) {
            found = s2(&format!("ARXIV:{id}")).map(|a| (a, "semantic_scholar"));
        }
    }
    if found.is_none() && !title.trim().is_empty() {
        let q = normalize(&title);
        found =
            s2(&format!("search/match?query={}", url_encode(&q))).map(|a| (a, "semantic_scholar"));
    }
    if found.is_none() {
        if let Some(d) = &doi {
            found = call(agent().get(&format!("https://api.crossref.org/works/{d}")))
                .ok()
                .and_then(|r| r.into_json::<Value>().ok())
                .and_then(|v| v["message"]["abstract"].as_str().map(strip_tags))
                // JATS 摘要常带 <jats:title>Abstract</jats:title>
                .map(|a| {
                    a.strip_prefix("Abstract")
                        .map(str::trim_start)
                        .unwrap_or(&a)
                        .to_owned()
                })
                .filter(|a| !a.is_empty())
                .map(|a| (a, "crossref"));
        }
    }
    CitedWork {
        key: e.key.clone(),
        title,
        authors,
        year: entry_year(e),
        source: found.as_ref().map(|(_, s)| *s),
        r#abstract: found.map(|(a, _)| a),
    }
}

fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            b' ' => "%20".into(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}
