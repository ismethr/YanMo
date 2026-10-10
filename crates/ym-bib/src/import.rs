//! 通过 DOI / arXiv ID 获取元数据。
//! Crossref: https://api.crossref.org/works/{doi}
//! arXiv:    http://export.arxiv.org/api/query?id_list={id}

/// 用户输入的标识符：DOI、arXiv ID，或包含它们的 URL。
pub enum Identifier {
    Doi(String),
    Arxiv(String),
}

pub fn detect(input: &str) -> Option<Identifier> {
    let s = input.trim();
    if let Some(i) = s.find("10.") {
        return Some(Identifier::Doi(s[i..].to_owned()));
    }
    let id = s
        .rsplit('/')
        .next()
        .unwrap_or(s)
        .trim_start_matches("arXiv:");
    if id.contains('.') && id.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Some(Identifier::Arxiv(id.to_owned()));
    }
    None
}
