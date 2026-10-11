//! stdio MCP Server：把命令注册表暴露给 Claude Code、Codex 等外部 Agent。
//!
//! - 每个对外开放的命令映射为一个 MCP tool：名称把 `.` 换成 `_`（`project.index` → `project_index`），
//!   因为部分客户端只接受 `[A-Za-z0-9_-]`；inputSchema 即命令的 params_schema。
//! - 不开放读写任意文件的命令（Agent 有自己的文件工具，改稿交给它们并由用户确认），
//!   也不开放返回大段 base64 的命令。会改动项目文件的命令标注 `readOnlyHint: false`，由客户端请用户确认。
//! - Agent 会直接改 .tex，所以每次调用前都丢弃索引缓存，保证结果反映磁盘上的最新内容。
//! - 协议消息逐行读写（JSON-RPC 2.0）；日志只写 stderr。

use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;
use ym_commands::{CommandMeta, Registry, Workspace};

/// 支持的协议版本，按新到旧排列；客户端请求的版本在列表中就沿用，否则回复最新的。
const PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

/// 不对外开放的命令
const HIDDEN: &[&str] = &[
    "app.set_lang",
    "file.read",
    "file.write",
    "file.base64",
    "compile.pdf",
];

/// 不改稿件、但会写文件的命令
const WRITES: &[&str] = &["export.docx", "project.set_main"];

const INSTRUCTIONS: &str = "YanMo gives structured access to a LaTeX paper project. \
Use project_index to see the outline, figures, tables and labels; check_run to find broken references, \
citations and missing files; compile_run to build the PDF and get errors with file and line. \
The .tex and .bib files are the source of truth: edit them with your own file tools, then run check_run \
or compile_run again to verify. Paths are relative to the project root.";

pub fn tool_name(id: &str) -> String {
    id.replace('.', "_")
}

fn exposed(m: &CommandMeta) -> bool {
    !HIDDEN.contains(&m.id)
}

pub struct Server {
    registry: Registry,
    ws: Workspace,
}

impl Server {
    /// `root` 为空时需由 Agent 调用 project_open 打开项目。
    pub fn new(root: Option<&Path>, main: Option<&Path>) -> anyhow::Result<Self> {
        let mut ws = Workspace::default();
        if let Some(root) = root {
            ws.open(root)?;
            if let Some(m) = main {
                ws.set_main(m, false)?;
            }
        }
        Ok(Self {
            registry: Registry::with_builtins(),
            ws,
        })
    }

    /// 处理一条消息；通知（没有 id）返回 `None`。
    pub fn handle(&mut self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);
        // 通知与客户端对我们请求的响应都不需要回复
        let id = id?;
        let result = match method {
            "initialize" => Ok(self.initialize(&params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": self.tools() })),
            "tools/call" => self.call(&params),
            "" => Err((-32600, "Invalid request".to_owned())),
            _ => Err((-32601, format!("Method not found: {method}"))),
        };
        Some(match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => {
                json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            }
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params["protocolVersion"].as_str().unwrap_or("");
        let version = PROTOCOL_VERSIONS
            .iter()
            .find(|v| **v == requested)
            .unwrap_or(&PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "yanmo", "title": "YanMo", "version": env!("CARGO_PKG_VERSION") },
            "instructions": INSTRUCTIONS,
        })
    }

    fn tools(&self) -> Vec<Value> {
        self.registry
            .list()
            .filter(|m| exposed(m))
            .map(|m| {
                let description = if m.description.is_empty() {
                    m.title_en
                } else {
                    m.description
                };
                json!({
                    "name": tool_name(m.id),
                    "title": m.title_en,
                    "description": description,
                    "inputSchema": m.params_schema,
                    "annotations": {
                        // 导出 Word 与设置主文件虽不改稿件，但会写文件
                        "readOnlyHint": !m.mutating && !WRITES.contains(&m.id),
                        "destructiveHint": false,
                        // 联网查询文献的命令会访问外部服务
                        "openWorldHint": matches!(m.id, "bib.fetch" | "bib.import"),
                    },
                })
            })
            .collect()
    }

    fn call(&mut self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params["name"]
            .as_str()
            .ok_or((-32602, "Missing tool name".to_owned()))?;
        let id = self
            .registry
            .list()
            .filter(|m| exposed(m))
            .find(|m| tool_name(m.id) == name)
            .map(|m| m.id)
            .ok_or_else(|| (-32602, format!("Unknown tool: {name}")))?;
        let args = match params.get("arguments") {
            Some(v) if v.is_object() => v.clone(),
            _ => json!({}),
        };
        self.ws.invalidate();
        // 命令本身的失败（找不到主文件、编译引擎缺失等）作为工具结果返回，让 Agent 能看到并处理
        Ok(match self.registry.execute(&mut self.ws, id, args) {
            Ok(out) => {
                let mut result = json!({
                    "content": [{ "type": "text", "text": serde_json::to_string_pretty(&out).unwrap_or_default() }],
                    "isError": false,
                });
                if out.is_object() {
                    result["structuredContent"] = out;
                }
                result
            }
            Err(e) => json!({
                "content": [{ "type": "text", "text": e.to_string() }],
                "isError": true,
            }),
        })
    }
}

/// 逐行读取请求并写回响应，直到输入结束。
pub fn serve(
    server: &mut Server,
    input: impl BufRead,
    mut output: impl Write,
) -> anyhow::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(_)) => Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32600, "message": "Batch requests are not supported" },
            })),
            Ok(msg) => server.handle(&msg),
            Err(e) => Some(json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("Parse error: {e}") },
            })),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}
