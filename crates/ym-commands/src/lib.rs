//! 统一命令注册表（借鉴 PhotoCraft）。
//!
//! 菜单、按钮、快捷键、命令面板、CLI、MCP 工具、AI Agent 都只通过这里执行操作，
//! 因此"用户能点的"与"Agent 能做的"天然一致，且都可被记录、撤销与测试。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// 命令 ID，约定 `<域>.<动作>`，例如 `compile.run`、`bib.import_doi`、`outline.get`。
pub type CommandId = &'static str;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMeta {
    pub id: String,
    /// i18n key，UI 据此取显示名
    pub title_key: String,
    /// 参数的 JSON Schema，同时用于 MCP tool 描述与 AI 工具调用
    pub params_schema: Value,
    /// 是否修改项目文件；修改类命令在 AI/MCP 调用时必须经用户确认
    pub mutating: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("unknown command: {0}")]
    Unknown(String),
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("{0}")]
    Failed(String),
}

/// 命令执行上下文，由宿主（desktop / cli / mcp）提供。
pub trait CommandContext: Send {
    fn project(&self) -> Option<&ym_model::Project>;
}

pub trait Command: Send + Sync {
    fn meta(&self) -> CommandMeta;
    fn run(&self, ctx: &mut dyn CommandContext, params: Value) -> Result<Value, CommandError>;
}

#[derive(Default)]
pub struct Registry {
    commands: BTreeMap<String, Box<dyn Command>>,
}

impl Registry {
    pub fn register(&mut self, cmd: Box<dyn Command>) {
        self.commands.insert(cmd.meta().id, cmd);
    }

    pub fn list(&self) -> impl Iterator<Item = CommandMeta> + '_ {
        self.commands.values().map(|c| c.meta())
    }

    pub fn execute(
        &self,
        ctx: &mut dyn CommandContext,
        id: &str,
        params: Value,
    ) -> Result<Value, CommandError> {
        let cmd = self
            .commands
            .get(id)
            .ok_or_else(|| CommandError::Unknown(id.to_owned()))?;
        cmd.run(ctx, params)
    }
}
