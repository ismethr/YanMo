//! 纯文本工具：标题/图注转纯文本、中英文混排字数统计。

/// 统计字数：中日韩文字逐字计数，其他语言按连续字母数字（含词内 `'` `-`）计为一词。
pub fn count_words(s: &str) -> u32 {
    let mut n = 0;
    let mut in_word = false;
    for ch in s.chars() {
        if is_cjk(ch) {
            n += 1;
            in_word = false;
        } else if ch.is_alphanumeric() {
            if !in_word {
                n += 1;
                in_word = true;
            }
        } else if !(in_word && (ch == '\'' || ch == '-')) {
            in_word = false;
        }
    }
    n
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3040..=0x30FF     // 平假名、片假名
        | 0x3400..=0x4DBF   // 扩展 A
        | 0x4E00..=0x9FFF   // 基本汉字
        | 0xAC00..=0xD7AF   // 韩文音节
        | 0xF900..=0xFAFF   // 兼容汉字
        | 0x20000..=0x2FA1F // 扩展 B 及以后
    )
}

/// 这些命令连同参数一起丢弃（转纯文本时没有可读内容）。
const DROP_WITH_ARGS: &[&str] = &[
    "label",
    "ref",
    "eqref",
    "cref",
    "Cref",
    "autoref",
    "pageref",
    "cite",
    "citep",
    "citet",
    "parencite",
    "textcite",
    "footnote",
    "thanks",
    "index",
    "protect",
];

/// 把一段 LaTeX 源码（标题、图注）转成便于显示的纯文本。
pub fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '%' => {
                while chars.peek().is_some_and(|&n| n != '\n') {
                    chars.next();
                }
            }
            '\\' => match chars.peek().copied() {
                Some(n) if n.is_ascii_alphabetic() => {
                    let mut name = String::new();
                    while let Some(&n) = chars.peek().filter(|n| n.is_ascii_alphabetic()) {
                        name.push(n);
                        chars.next();
                    }
                    match name.as_str() {
                        "LaTeX" | "TeX" => out.push_str(&name),
                        _ if DROP_WITH_ARGS.contains(&name.as_str()) => skip_args(&mut chars),
                        _ => {}
                    }
                }
                Some(n) => {
                    chars.next();
                    out.push(if "%&_#${}".contains(n) { n } else { ' ' });
                }
                None => {}
            },
            '{' | '}' => {}
            '~' => out.push(' '),
            c => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 跳过紧随其后的 `*`、`[...]` 与 `{...}` 参数。
fn skip_args(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    if chars.peek() == Some(&'*') {
        chars.next();
    }
    while let Some(&open) = chars.peek().filter(|c| **c == '[' || **c == '{') {
        let close = if open == '[' { ']' } else { '}' };
        chars.next();
        let mut depth = 0;
        for c in chars.by_ref() {
            match c {
                '{' => depth += 1,
                '}' if depth > 0 => depth -= 1,
                c if c == close && depth == 0 => break,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_mixed_text() {
        assert_eq!(count_words("Hello, world"), 2);
        assert_eq!(count_words("well-known don't"), 2);
        assert_eq!(count_words("Transformer 改变了序列建模。"), 1 + 7);
        assert_eq!(count_words("  ，。 "), 0);
    }

    #[test]
    fn plain_strips_markup() {
        assert_eq!(plain(r"Our \textbf{Fast} Model~\cite{x}"), "Our Fast Model");
        assert_eq!(plain(r"整体架构\label{fig:a}"), "整体架构");
        assert_eq!(
            plain(r"50\% of \LaTeX{} users % comment"),
            "50% of LaTeX users"
        );
    }
}
