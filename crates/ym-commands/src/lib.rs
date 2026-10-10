//! 统一命令注册表（借鉴 PhotoCraft）。
//!
//! 菜单、按钮、快捷键、命令面板、CLI、MCP 工具、AI Agent 都只通过这里执行操作，
//! 因此"用户能点的"与"Agent 能做的"天然一致，且都可被记录、撤销与测试。

mod builtin;
mod workspace;

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

pub use workspace::{CompileJob, ExportJob, Workspace};

#[derive(Debug, Clone, Serialize)]
pub struct CommandMeta {
    /// 约定 `<域>.<动作>`，例如 `project.open`、`check.run`
    pub id: &'static str,
    /// 显示名（i18n 之前先用中文）
    pub title: &'static str,
    /// 参数的 JSON Schema，同时用于 MCP tool 描述与 AI 工具调用
    pub params_schema: Value,
    /// 是否修改项目文件；修改类命令在 AI/MCP 调用时必须经用户确认
    pub mutating: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("未知命令：{0}")]
    Unknown(String),
    #[error("参数错误：{0}")]
    InvalidParams(String),
    #[error("尚未打开项目")]
    NoProject,
    #[error("{0}")]
    Failed(String),
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        CommandError::Failed(e.to_string())
    }
}

pub type CommandResult = Result<Value, CommandError>;
type Handler = fn(&mut Workspace, Value) -> CommandResult;

struct Entry {
    meta: CommandMeta,
    handler: Handler,
}

#[derive(Default)]
pub struct Registry {
    commands: BTreeMap<&'static str, Entry>,
}

impl Registry {
    /// 注册了全部内置命令的注册表。
    pub fn with_builtins() -> Self {
        let mut r = Self::default();
        builtin::register(&mut r);
        r
    }

    pub fn register(&mut self, meta: CommandMeta, handler: Handler) {
        self.commands.insert(meta.id, Entry { meta, handler });
    }

    pub fn list(&self) -> impl Iterator<Item = &CommandMeta> {
        self.commands.values().map(|e| &e.meta)
    }

    pub fn execute(&self, ws: &mut Workspace, id: &str, params: Value) -> CommandResult {
        let entry = self
            .commands
            .get(id)
            .ok_or_else(|| CommandError::Unknown(id.to_owned()))?;
        (entry.handler)(ws, params)
    }
}
