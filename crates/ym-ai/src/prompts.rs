//! 写作任务的提示词。界面只传任务类型与上下文，提示词集中在这里，CLI / MCP 复用同一套。

use serde::Deserialize;

use crate::{ChatRequest, Message, Role};

const BASE: &str = "You are the writing assistant built into YanMo, a LaTeX editor for academic papers. \
You help researchers write clear, precise, publication-ready prose.

Rules for anything you write back into the manuscript:
- Keep it valid LaTeX. Preserve every command, environment, math expression, \\cite{...}, \\ref{...}, \\label{...} key and comment exactly unless the user asks to change them.
- Do not invent citations, data, numbers or findings. If something needs a source, say so instead.
- Match the manuscript's language, spelling variant and terminology.
When you chat, answer in the language the user writes in, concisely. Use LaTeX in code blocks when showing source.";

/// 选区改写的种类。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RewriteAction {
    Polish,
    Academic,
    TranslateToEnglish,
    TranslateToChinese,
    Shorten,
    Expand,
}

impl RewriteAction {
    fn instruction(self) -> &'static str {
        match self {
            RewriteAction::Polish => "Polish the passage: fix grammar, awkward phrasing and unclear sentences while keeping the meaning, tone and length close to the original.",
            RewriteAction::Academic => "Rewrite the passage in a more formal academic register suitable for a peer-reviewed journal, keeping the meaning and claims unchanged.",
            RewriteAction::TranslateToEnglish => "Translate the passage into fluent academic English.",
            RewriteAction::TranslateToChinese => "将这段内容翻译为流畅、规范的学术中文。",
            RewriteAction::Shorten => "Make the passage more concise (about 30% shorter) without dropping any claim, number or citation.",
            RewriteAction::Expand => "Expand the passage with clearer explanation and transitions (about 30% longer), without adding new claims, data or citations.",
        }
    }
}

/// 界面附带的上下文。只包含用户可见、并在发送前展示过的内容。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TaskContext {
    pub file: Option<String>,
    pub section_title: Option<String>,
    /// 当前章节的源码（用作理解上下文，不要求改写）
    pub section_text: Option<String>,
    /// 论文大纲（章节标题列表）
    pub outline: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Task {
    /// 改写选区，结果以 diff 呈现
    Rewrite {
        action: RewriteAction,
        selection: String,
    },
    /// 自由对话（含此前的多轮消息，最后一条为用户的新消息）
    Chat { history: Vec<Message> },
    /// 解释编译错误并给出修改建议
    ExplainError { diagnostics: String, source: String },
}

fn context_block(ctx: &TaskContext) -> String {
    let mut s = String::new();
    if let Some(o) = ctx.outline.as_deref().filter(|o| !o.is_empty()) {
        s.push_str(&format!("<outline>\n{o}\n</outline>\n"));
    }
    if let Some(f) = &ctx.file {
        s.push_str(&format!("<current_file>{f}</current_file>\n"));
    }
    if let Some(text) = ctx.section_text.as_deref().filter(|t| !t.is_empty()) {
        let title = ctx.section_title.as_deref().unwrap_or("");
        s.push_str(&format!(
            "<current_section title=\"{title}\">\n{text}\n</current_section>\n"
        ));
    }
    s
}

/// 从模型回复中取出改写结果：优先取第一个 ``` 代码块，否则取全文。
pub fn extract_rewrite(reply: &str) -> String {
    if let Some(start) = reply.find("```") {
        let after = &reply[start + 3..];
        let body_start = after.find('\n').map(|i| i + 1).unwrap_or(0);
        let body = &after[body_start..];
        if let Some(end) = body.find("```") {
            return body[..end].trim_end_matches('\n').to_owned();
        }
    }
    reply.trim().to_owned()
}

pub fn build(task: &Task, ctx: &TaskContext) -> ChatRequest {
    let context = context_block(ctx);
    match task {
        Task::Rewrite { action, selection } => ChatRequest {
            system: BASE.to_owned(),
            messages: vec![Message {
                role: Role::User,
                content: format!(
                    "{context}{}\n\nReturn only the rewritten LaTeX inside a single ```latex code block, with no explanation before or after it.\n\n<passage>\n{selection}\n</passage>",
                    action.instruction()
                ),
            }],
        },
        Task::Chat { history } => {
            let mut messages = history.clone();
            // 上下文附在最近一条用户消息前，避免改动历史消息（便于缓存与思考块复用）
            if let Some(last) = messages.iter_mut().rev().find(|m| m.role == Role::User) {
                if !context.is_empty() {
                    last.content = format!("{context}\n{}", last.content);
                }
            }
            ChatRequest { system: BASE.to_owned(), messages }
        }
        Task::ExplainError { diagnostics, source } => ChatRequest {
            system: BASE.to_owned(),
            messages: vec![Message {
                role: Role::User,
                content: format!(
                    "{context}The LaTeX build reported these problems:\n<diagnostics>\n{diagnostics}\n</diagnostics>\n\nRelevant source (with line numbers):\n<source>\n{source}\n</source>\n\nExplain the likely cause in plain language and show the minimal fix as a LaTeX code block. Answer in Chinese if the manuscript is Chinese, otherwise in the user's interface language (Chinese)."
                ),
            }],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fenced_rewrite() {
        assert_eq!(
            extract_rewrite("Here:\n```latex\nA \\cite{x}.\n```\nDone"),
            "A \\cite{x}."
        );
        assert_eq!(extract_rewrite("plain text"), "plain text");
    }

    #[test]
    fn chat_context_goes_on_latest_user_message() {
        let ctx = TaskContext {
            section_title: Some("Intro".into()),
            section_text: Some("Body".into()),
            ..Default::default()
        };
        let history = vec![
            Message {
                role: Role::User,
                content: "first".into(),
            },
            Message {
                role: Role::Assistant,
                content: "reply".into(),
            },
            Message {
                role: Role::User,
                content: "second".into(),
            },
        ];
        let req = build(&Task::Chat { history }, &ctx);
        assert_eq!(req.messages[0].content, "first");
        assert!(req.messages[2]
            .content
            .contains("<current_section title=\"Intro\">"));
        assert!(req.messages[2].content.ends_with("second"));
    }
}
