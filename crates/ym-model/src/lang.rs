//! 界面语言。
//!
//! 进程内只有一个当前语言：桌面应用在启动和切换语言时设置，命令行按 `--lang` 或系统语言设置。
//! 需要给用户看的文字用 [`tr!`](crate::tr) 同时写出中英文两份。
//! 论文本身的语言（例如导出 Word 时的“图 1”/“Figure 1”）由论文内容决定，与这里无关。

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    #[default]
    Zh,
    En,
}

impl Lang {
    /// 解析语言标签，如 `zh`、`zh-CN`、`en_US.UTF-8`；不认识的返回 `None`。
    pub fn parse(tag: &str) -> Option<Lang> {
        let tag = tag.trim().to_ascii_lowercase();
        if tag.starts_with("zh") {
            Some(Lang::Zh)
        } else if tag.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }

    /// 从系统环境变量推断（LC_ALL → LC_MESSAGES → LANG）；中文环境用中文，其他一律英文。
    pub fn from_env() -> Lang {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(var) {
                if v.is_empty() || v == "C" || v == "POSIX" {
                    continue;
                }
                return Lang::parse(&v).unwrap_or(Lang::En);
            }
        }
        Lang::Zh
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::En as u8 {
        Lang::En
    } else {
        Lang::Zh
    }
}

/// 按当前语言选择文字：`tr!("找不到 {path}", "Cannot find {path}")`。
/// 两份都是 `format!` 格式串，只格式化被选中的那一份。
#[macro_export]
macro_rules! tr {
    ($zh:literal, $en:literal $(,)?) => {
        match $crate::lang::lang() {
            $crate::lang::Lang::Zh => ::std::string::String::from(::std::format!($zh)),
            $crate::lang::Lang::En => ::std::string::String::from(::std::format!($en)),
        }
    };
    ($zh:literal, $en:literal, $($arg:tt)+) => {
        match $crate::lang::lang() {
            $crate::lang::Lang::Zh => ::std::format!($zh, $($arg)+),
            $crate::lang::Lang::En => ::std::format!($en, $($arg)+),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tags() {
        assert_eq!(Lang::parse("zh-CN"), Some(Lang::Zh));
        assert_eq!(Lang::parse("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::parse("fr"), None);
    }
}
