//! 从本机 Zotero 取文献：通过 Zotero 7 起提供的本地 API（http://127.0.0.1:23119/api/），
//! 需要在 Zotero 设置 → 高级 中允许其他应用程序与 Zotero 通讯。不需要 Better BibTeX 插件。
//!
//! 导出的 BibTeX 会清理掉本机附件路径、个人笔记、摘要等不应进入论文的字段，并按研墨的规则重新生成 key。

use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

use ym_model::{tr, BibEntry};

use crate::{make_key, parse, BibError};

const BASE: &str = "http://127.0.0.1:23119";

/// Zotero 导出中不应带进论文 .bib 的字段：本机附件路径、个人笔记、摘要、关键词、访问日期等
const DROP: &[&str] = &[
    "file",
    "annote",
    "abstract",
    "keywords",
    "note",
    "urldate",
    "copyright",
    "shorttitle",
    "issn",
    "isbn",
    "language",
    "langid",
];

#[derive(Debug, Clone, Serialize)]
pub struct ZoteroItem {
    /// Zotero 条目 key（不是引用 key）
    pub key: String,
    pub title: String,
    /// 简短作者：Liu et al.
    pub authors: String,
    pub year: Option<String>,
    pub item_type: String,
    pub doi: Option<String>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .build()
}

fn err(e: ureq::Error) -> BibError {
    match e {
        ureq::Error::Transport(_) => BibError::Network(tr!(
            "无法连接 Zotero：请确认 Zotero 正在运行",
            "Cannot reach Zotero: make sure Zotero is running"
        )),
        ureq::Error::Status(403, _) => BibError::Network(tr!(
            "Zotero 的本地 API 未开启：请在 Zotero 设置 → 高级 中允许其他应用程序与 Zotero 通讯",
            "Zotero's local API is off: in Zotero Settings → Advanced, allow other applications to communicate with Zotero"
        )),
        other => BibError::Network(other.to_string()),
    }
}

fn short_authors(creators: &Value) -> String {
    let list: Vec<&Value> = creators
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|c| c["creatorType"].as_str().is_none_or(|t| t == "author"))
                .collect()
        })
        .unwrap_or_default();
    let name = |c: &Value| {
        c["lastName"]
            .as_str()
            .or(c["name"].as_str())
            .unwrap_or("")
            .to_owned()
    };
    match list.len() {
        0 => String::new(),
        1 => name(list[0]),
        2 => format!("{} & {}", name(list[0]), name(list[1])),
        _ => format!("{} et al.", name(list[0])),
    }
}

/// 解析本地 API 的条目 JSON
pub fn parse_item(v: &Value) -> ZoteroItem {
    let d = &v["data"];
    let date = d["date"].as_str().unwrap_or("");
    ZoteroItem {
        key: v["key"].as_str().unwrap_or("").to_owned(),
        title: d["title"].as_str().unwrap_or("").to_owned(),
        authors: short_authors(&d["creators"]),
        year: date
            .split(|c: char| !c.is_ascii_digit())
            .find(|p| p.len() == 4)
            .map(str::to_owned),
        item_type: d["itemType"].as_str().unwrap_or("").to_owned(),
        doi: d["DOI"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    }
}

/// 按标题、作者、年份搜索文献库（不含附件与笔记）
pub fn search(query: &str, limit: u32) -> Result<Vec<ZoteroItem>, BibError> {
    let v: Value = agent()
        .get(&format!("{BASE}/api/users/0/items/top"))
        .query("q", query)
        .query("qmode", "titleCreatorYear")
        .query("itemType", "-attachment")
        .query("limit", &limit.to_string())
        .query("format", "json")
        .call()
        .map_err(err)?
        .into_json()
        .map_err(|e| BibError::Network(e.to_string()))?;
    // 独立笔记也不是文献
    Ok(v.as_array()
        .map(|a| {
            a.iter()
                .map(parse_item)
                .filter(|i| i.item_type != "note")
                .collect()
        })
        .unwrap_or_default())
}

/// 清理 Zotero 导出的条目：去掉不应进入论文的字段，有 DOI 时去掉重复的 URL，重新生成 key
pub fn clean(mut entry: BibEntry) -> BibEntry {
    entry.fields.retain(|k, _| !DROP.contains(&k.as_str()));
    if entry.fields.contains_key("doi") {
        entry.fields.remove("url");
    }
    entry.key = make_key(&entry);
    entry
}

/// 导出指定条目为 BibTeX 并清理
pub fn export(keys: &[String]) -> Result<Vec<BibEntry>, BibError> {
    let mut out = Vec::new();
    for key in keys {
        let text = agent()
            .get(&format!("{BASE}/api/users/0/items"))
            .query("itemKey", key)
            .query("format", "bibtex")
            .call()
            .map_err(err)?
            .into_string()
            .map_err(|e| BibError::Network(e.to_string()))?;
        if let Some(e) = parse(Path::new("zotero.bib"), &text)
            .entries
            .into_iter()
            .next()
        {
            out.push(clean(e));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_exported_entry() {
        let src = "@article{_meta_2023,\n\ttitle = {{中国主粮作物生物炭产量效应的Meta分析}},\n\turl = {https://doi.org/10.13227/x},\n\tdoi = {10.13227/x},\n\tabstract = {摘要},\n\tjournal = {环境科学},\n\tauthor = {{于滨杭} and {姬建梅}},\n\tyear = {2023},\n\tnote = {CNKICite: 45},\n\tannote = {个人笔记},\n\tfile = {全文:/Users/me/Zotero/storage/X/a.pdf:application/pdf},\n}\n";
        let e = clean(parse(Path::new("z.bib"), src).entries.remove(0));
        assert_eq!(e.key, "yu2023meta");
        let fields: Vec<&str> = e.fields.keys().map(String::as_str).collect();
        assert_eq!(fields, ["author", "doi", "journal", "title", "year"]);
    }

    #[test]
    fn parses_search_item() {
        let v: Value = serde_json::from_str(r#"{"key":"H326ZYKU","data":{"itemType":"journalArticle","title":"Biochar's effect","creators":[{"creatorType":"author","firstName":"Xiaoyu","lastName":"Liu"},{"creatorType":"author","firstName":"Afeng","lastName":"Zhang"},{"creatorType":"author","lastName":"Ji"}],"date":"2013-08","DOI":"10.1007/s11104-013-1806-x"}}"#).unwrap();
        let i = parse_item(&v);
        assert_eq!(i.authors, "Liu et al.");
        assert_eq!(i.year.as_deref(), Some("2013"));
    }

    #[test]
    #[ignore = "需要本机 Zotero"]
    fn live_search_and_export() {
        let items = search("biochar", 5).unwrap();
        eprintln!("{items:#?}");
        assert!(!items.is_empty());
        let entries = export(&[items[0].key.clone()]).unwrap();
        eprintln!("{}", crate::import::to_bibtex(&entries[0]));
        assert!(entries[0].field("file").is_none());
    }
}
