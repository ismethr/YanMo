use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use ym_mcp::{serve, Server};

fn sample() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/sample-paper")
}

/// 把示例论文复制到临时目录，测试可以随意改动
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ym-mcp-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy_dir(&sample(), &dir);
    dir
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(name));
        } else {
            fs::copy(e.path(), to.join(name)).unwrap();
        }
    }
}

/// 发送一组消息，返回按行解析的回复
fn exchange(server: &mut Server, msgs: &[Value]) -> Vec<Value> {
    let input: String = msgs.iter().map(|m| format!("{m}\n")).collect();
    let mut out = Vec::new();
    serve(server, input.as_bytes(), &mut out).unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn call(id: u32, name: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": name, "arguments": args}})
}

#[test]
fn handshake_and_tool_list() {
    let mut s = Server::new(Some(&sample()), None).unwrap();
    let r = exchange(
        &mut s,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "t", "version": "0"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "nope"}),
        ],
    );
    // 通知没有回复
    assert_eq!(r.len(), 3);
    assert_eq!(r[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r[0]["result"]["serverInfo"]["name"], "yanmo");

    let tools = r[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"project_index"));
    assert!(names.contains(&"check_run"));
    assert!(!names.contains(&"file_write"), "任意写文件不对外开放");
    for t in tools {
        let name = t["name"].as_str().unwrap();
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "{name}"
        );
        assert_eq!(t["inputSchema"]["type"], "object", "{name}");
        assert!(!t["description"].as_str().unwrap().is_empty(), "{name}");
    }
    let bib_add = tools.iter().find(|t| t["name"] == "bib_add").unwrap();
    assert_eq!(bib_add["annotations"]["readOnlyHint"], false);

    assert_eq!(r[2]["error"]["code"], -32601);
}

#[test]
fn unknown_protocol_version_gets_latest() {
    let mut s = Server::new(None, None).unwrap();
    let r = exchange(
        &mut s,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}),
        ],
    );
    assert_eq!(r[0]["result"]["protocolVersion"], "2025-11-25");
}

#[test]
fn sees_edits_made_between_calls() {
    let dir = scratch("edits");
    let mut s = Server::new(Some(&dir), None).unwrap();
    let before = exchange(&mut s, &[call(1, "check_run", json!({}))]);
    let errors = before[0]["result"]["structuredContent"]["errors"]
        .as_u64()
        .unwrap();
    assert!(errors > 0);

    // 模拟 Agent 用自己的文件工具修好一个未定义引用
    let intro = dir.join("sections/intro.tex");
    let text = fs::read_to_string(&intro).unwrap();
    fs::write(
        &intro,
        text.replace("\\ref{fig:missing}", "\\ref{fig:arch}"),
    )
    .unwrap();

    let after = exchange(&mut s, &[call(2, "check_run", json!({}))]);
    let res = &after[0]["result"];
    assert_eq!(res["isError"], false);
    assert_eq!(
        res["structuredContent"]["errors"].as_u64().unwrap(),
        errors - 1
    );
    // 文本内容与结构化内容一致
    let text: Value = serde_json::from_str(res["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, res["structuredContent"]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn command_failures_are_tool_errors() {
    // 没有打开项目时调用：作为工具结果返回错误，而不是协议错误
    let mut s = Server::new(None, None).unwrap();
    let r = exchange(&mut s, &[call(1, "project_index", json!({}))]);
    assert_eq!(r[0]["result"]["isError"], true);
    assert!(r[0].get("error").is_none());

    // 打开项目后可以正常使用
    let r = exchange(
        &mut s,
        &[
            call(2, "project_open", json!({"path": sample()})),
            call(3, "project_index", json!({})),
        ],
    );
    assert_eq!(r[0]["result"]["isError"], false);
    let outline = &r[1]["result"]["structuredContent"]["outline"];
    assert_eq!(outline[0]["label"], "sec:intro");
}

#[test]
fn malformed_input_does_not_stop_the_server() {
    let mut s = Server::new(None, None).unwrap();
    let input = "not json\n[1,2]\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"ping\"}\n";
    let mut out = Vec::new();
    serve(&mut s, input.as_bytes(), &mut out).unwrap();
    let lines: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines[0]["error"]["code"], -32700);
    assert_eq!(lines[1]["error"]["code"], -32600);
    assert_eq!(lines[2]["id"], 7);
    assert_eq!(lines[2]["result"], json!({}));
}
