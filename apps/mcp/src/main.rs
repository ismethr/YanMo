//! stdio MCP Server。
//! 每个 `CommandMeta` 映射为一个 MCP tool（name = 命令 ID，inputSchema = params_schema）。
//! `mutating = true` 的命令默认只返回补丁预览，需桌面端确认后才会落盘。

fn main() -> anyhow::Result<()> {
    todo!("P1")
}
