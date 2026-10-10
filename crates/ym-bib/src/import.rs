//! 通过 DOI / arXiv ID 获取文献元数据，生成规范的 BibTeX 条目。
//!
//! - DOI：向 `https://doi.org/<doi>` 请求 `application/x-bibtex`（Crossref、DataCite 等注册机构都支持）；
//! - arXiv：`https://export.arxiv.org/api/query?id_list=<id>`（Atom），有正式发表 DOI 时改走 DOI。
//!
//! 取回后重新生成引用 key（`作者年份标题词`），并按固定字段顺序输出，避免各来源格式不一。

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use ym_model::{BibEntry, SourceSpan};

use crate::{make_key, parse, BibError};

/// 用户输入的标识符：DOI、arXiv ID，或包含它们的 URL。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identifier {
    Doi(String),
    Arxiv(String),
}

fn arxiv_new(s: &str) -> Option<String> {
    // 2401.12345、2401.12345v2
    let (year_month, rest) = s.split_once('.')?;
    let (num, version) = match rest.split_once('v') {
        Some((n, v)) => (n, Some(v)),
        None => (rest, None),
    };
    let digits = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
    let ok = year_month.len() == 4
        && digits(year_month)
        && (4..=5).contains(&num.len())
        && digits(num)
        && version.is_none_or(digits);
    ok.then(|| format!("{year_month}.{num}"))
}

/// 识别输入中的标识符。支持：`10.xxxx/yyy`、`doi:…`、`https://doi.org/…`、`arXiv:2401.12345`、
/// `https://arxiv.org/abs/2401.12345v2`、旧式 `hep-th/9901001`，以及 arXiv 的 DOI `10.48550/arXiv.…`。
pub fn detect(input: &str) -> Option<Identifier> {
    let s = input.trim().trim_end_matches(['.', ',', ';']);
    let lower = s.to_ascii_lowercase();

    // arXiv 链接或前缀
    for marker in [
        "arxiv.org/abs/",
        "arxiv.org/pdf/",
        "arxiv:",
        "10.48550/arxiv.",
    ] {
        if let Some(i) = lower.find(marker) {
            let id = s[i + marker.len()..]
                .trim_end_matches(".pdf")
                .trim_matches('/');
            return arxiv_new(id)
                .or_else(|| id.contains('/').then(|| id.to_owned()))
                .map(Identifier::Arxiv);
        }
    }
    // DOI：10.<注册号>/<后缀>
    if let Some(i) = lower.find("10.") {
        let doi = s[i..].split_whitespace().next().unwrap_or_default();
        if let Some((prefix, suffix)) = doi.split_once('/') {
            let reg = &prefix[3..];
            if (4..=9).contains(&reg.len())
                && reg.chars().all(|c| c.is_ascii_digit() || c == '.')
                && !suffix.is_empty()
            {
                return Some(Identifier::Doi(doi.to_owned()));
            }
        }
    }
    // 纯 arXiv 编号
    arxiv_new(s).map(Identifier::Arxiv)
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(20))
        .user_agent(concat!(
            "YanMo/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/ismethr/YanMo)"
        ))
        .build()
}

fn net(e: impl std::fmt::Display) -> BibError {
    BibError::Network(e.to_string())
}

/// 取回 DOI 对应的 BibTeX。
fn fetch_doi(doi: &str) -> Result<BibEntry, BibError> {
    let text = agent()
        .get(&format!("https://doi.org/{doi}"))
        .set("Accept", "application/x-bibtex; charset=utf-8")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => BibError::Network(format!("找不到 DOI：{doi}")),
            other => net(other),
        })?
        .into_string()
        .map_err(net)?;
    let out = parse(Path::new("doi.bib"), &text);
    let mut entry = out
        .entries
        .into_iter()
        .next()
        .ok_or_else(|| BibError::Network(format!("DOI {doi} 返回的不是 BibTeX")))?;
    entry
        .fields
        .entry("doi".into())
        .or_insert_with(|| doi.to_owned());
    // Crossref 会附带很长的 url 字段（就是 doi.org 链接），有 doi 时去掉
    if entry
        .fields
        .get("url")
        .is_some_and(|u| u.contains("doi.org"))
    {
        entry.fields.remove("url");
    }
    Ok(entry)
}

fn xml_text<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let i = xml.find(&open)?;
    let start = i + xml[i..].find('>')? + 1;
    let end = start + xml[start..].find(&format!("</{tag}>"))?;
    Some(xml[start..end].trim())
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// 解析 arXiv API 返回的 Atom。返回条目以及正式发表的 DOI（如有）。
pub fn parse_arxiv_atom(xml: &str, id: &str) -> Result<(BibEntry, Option<String>), BibError> {
    let entry = xml_text(xml, "entry")
        .ok_or_else(|| BibError::Network(format!("arXiv 中没有找到 {id}")))?;
    let title = xml_text(entry, "title")
        .map(|t| unescape(&t.split_whitespace().collect::<Vec<_>>().join(" ")));
    if title
        .as_deref()
        .is_none_or(|t| t.is_empty() || t == "Error")
    {
        return Err(BibError::Network(format!("arXiv 中没有找到 {id}")));
    }
    let mut authors = Vec::new();
    let mut rest = entry;
    while let Some(i) = rest.find("<author>") {
        let block = &rest[i..];
        if let Some(name) = xml_text(block, "name") {
            authors.push(unescape(name));
        }
        rest = &block[8..];
    }
    let year = xml_text(entry, "published").map(|p| p.chars().take(4).collect::<String>());
    let doi = xml_text(entry, "arxiv:doi").map(str::to_owned);
    let category = entry.find("<arxiv:primary_category").and_then(|i| {
        let tag = &entry[i..];
        let j = tag.find("term=\"")? + 6;
        Some(tag[j..j + tag[j..].find('"')?].to_owned())
    });

    let mut fields = BTreeMap::new();
    if let Some(t) = title {
        fields.insert("title".into(), t);
    }
    if !authors.is_empty() {
        fields.insert("author".into(), authors.join(" and "));
    }
    if let Some(y) = year {
        fields.insert("year".into(), y);
    }
    fields.insert("eprint".into(), id.to_owned());
    fields.insert("archiveprefix".into(), "arXiv".into());
    if let Some(c) = category {
        fields.insert("primaryclass".into(), c);
    }
    fields.insert("url".into(), format!("https://arxiv.org/abs/{id}"));
    let entry = BibEntry {
        key: String::new(),
        entry_type: "misc".into(),
        fields,
        span: SourceSpan {
            file: "arxiv".into(),
            start: 0,
            end: 0,
            line: 0,
        },
        citations: Vec::new(),
    };
    Ok((entry, doi))
}

fn fetch_arxiv(id: &str) -> Result<BibEntry, BibError> {
    let xml = agent()
        .get("https://export.arxiv.org/api/query")
        .query("id_list", id)
        .call()
        .map_err(net)?
        .into_string()
        .map_err(net)?;
    let (mut entry, doi) = parse_arxiv_atom(&xml, id)?;
    // 已正式发表：以期刊版本为准，同时保留 arXiv 信息
    if let Some(doi) = doi {
        if let Ok(mut published) = fetch_doi(&doi) {
            for k in ["eprint", "archiveprefix", "primaryclass"] {
                if let Some(v) = entry.fields.remove(k) {
                    published.fields.insert(k.into(), v);
                }
            }
            return Ok(published);
        }
    }
    entry.key.clear();
    Ok(entry)
}

/// 取回元数据并生成规范的条目（key 已重新生成）。
pub fn fetch(id: &Identifier) -> Result<BibEntry, BibError> {
    let mut entry = match id {
        Identifier::Doi(d) => fetch_doi(d)?,
        Identifier::Arxiv(a) => fetch_arxiv(a)?,
    };
    entry.entry_type = entry.entry_type.to_ascii_lowercase();
    entry.key = make_key(&entry);
    if entry.key.is_empty() {
        entry.key = "ref".into();
    }
    Ok(entry)
}

/// 字段的输出顺序；其余字段按字母序排在后面。
const FIELD_ORDER: &[&str] = &[
    "author",
    "title",
    "journal",
    "booktitle",
    "year",
    "volume",
    "number",
    "pages",
    "publisher",
    "doi",
    "url",
    "eprint",
    "archiveprefix",
    "primaryclass",
];

/// 输出为 BibTeX 文本。
pub fn to_bibtex(entry: &BibEntry) -> String {
    let mut keys: Vec<&String> = entry.fields.keys().collect();
    keys.sort_by_key(|k| {
        (
            FIELD_ORDER
                .iter()
                .position(|f| f == k)
                .unwrap_or(FIELD_ORDER.len()),
            k.as_str(),
        )
    });
    let width = keys.iter().map(|k| k.len()).max().unwrap_or(0);
    let mut out = format!("@{}{{{},\n", entry.entry_type, entry.key);
    for k in keys {
        let v = &entry.fields[k];
        out.push_str(&format!("  {k:<width$} = {{{v}}},\n"));
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_identifiers() {
        let doi = |s: &str| Some(Identifier::Doi(s.into()));
        let ax = |s: &str| Some(Identifier::Arxiv(s.into()));
        assert_eq!(detect("10.1109/CVPR.2016.90"), doi("10.1109/CVPR.2016.90"));
        assert_eq!(
            detect("https://doi.org/10.1016/j.apgeog.2024.103201"),
            doi("10.1016/j.apgeog.2024.103201")
        );
        assert_eq!(
            detect("doi: 10.1038/nature14539."),
            doi("10.1038/nature14539")
        );
        // arXiv 编号里含有 "10." 也不能误判为 DOI
        assert_eq!(detect("2310.01234"), ax("2310.01234"));
        assert_eq!(
            detect("https://arxiv.org/abs/2310.01234v3"),
            ax("2310.01234")
        );
        assert_eq!(detect("arXiv:1706.03762"), ax("1706.03762"));
        assert_eq!(
            detect("https://arxiv.org/pdf/1706.03762v7.pdf"),
            ax("1706.03762")
        );
        assert_eq!(detect("10.48550/arXiv.1706.03762"), ax("1706.03762"));
        assert_eq!(detect("hep-th/9901001"), None);
        assert_eq!(detect("arXiv:hep-th/9901001"), ax("hep-th/9901001"));
        assert_eq!(detect("not an id"), None);
    }

    #[test]
    fn parses_arxiv_atom_and_writes_bibtex() {
        let xml = r#"<feed><title>ArXiv Query</title><entry>
            <id>http://arxiv.org/abs/1706.03762v7</id>
            <published>2017-06-12T17:57:34Z</published>
            <title>Attention Is All
              You Need</title>
            <author><name>Ashish Vaswani</name></author>
            <author><name>Noam Shazeer</name></author>
            <arxiv:primary_category xmlns:arxiv="http://arxiv.org/schemas/atom" term="cs.CL" scheme="x"/>
        </entry></feed>"#;
        let (mut e, doi) = parse_arxiv_atom(xml, "1706.03762").unwrap();
        assert_eq!(doi, None);
        assert_eq!(e.field("title"), Some("Attention Is All You Need"));
        assert_eq!(e.field("author"), Some("Ashish Vaswani and Noam Shazeer"));
        assert_eq!(e.field("year"), Some("2017"));
        assert_eq!(e.field("primaryclass"), Some("cs.CL"));
        e.key = make_key(&e);
        assert_eq!(e.key, "vaswani2017attention");
        let bib = to_bibtex(&e);
        assert!(bib.starts_with("@misc{vaswani2017attention,\n  author "));
        let back = parse(Path::new("x.bib"), &bib);
        assert!(back.errors.is_empty());
        assert_eq!(back.entries[0].field("eprint"), Some("1706.03762"));
    }

    #[test]
    #[ignore = "需要网络"]
    fn fetches_real_doi() {
        let e = fetch(&Identifier::Doi("10.1109/CVPR.2016.90".into())).unwrap();
        assert_eq!(e.key, "he2016deep");
    }
}
