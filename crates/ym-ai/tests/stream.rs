//! 用本地模拟服务器验证流式解析：按 Anthropic / OpenAI 的 SSE 格式返回固定内容。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use ym_ai::{AiError, AiSettings, ChatRequest, Message, ProviderKind, Role, StreamEvent};

/// 启动只处理一个请求的服务器；返回地址与收到的请求（请求行+头+体）。
fn serve(status: &str, body: String) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let status = status.to_owned();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut head = String::new();
        let mut len = 0usize;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                len = v.trim().parse().unwrap();
            }
            head.push_str(&line);
            if line == "\r\n" {
                break;
            }
        }
        let mut buf = vec![0; len];
        reader.read_exact(&mut buf).unwrap();
        tx.send(head + &String::from_utf8(buf).unwrap()).unwrap();
        let content_type = if status.starts_with("200") {
            "text/event-stream"
        } else {
            "application/json"
        };
        write!(
            stream,
            "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (addr, rx)
}

fn sse(events: &[&str]) -> String {
    events
        .iter()
        .map(|e| format!("event: x\ndata: {e}\n\n"))
        .collect()
}

fn req() -> ChatRequest {
    ChatRequest {
        system: "sys".into(),
        messages: vec![Message {
            role: Role::User,
            content: "hi".into(),
        }],
    }
}

fn run(settings: &AiSettings) -> (Result<ym_ai::ChatOutcome, AiError>, Vec<StreamEvent>) {
    let mut events = Vec::new();
    let r = ym_ai::stream(settings, &req(), &AtomicBool::new(false), &mut |e| {
        events.push(e)
    });
    (r, events)
}

// 环境变量是进程级的，Anthropic 相关的场景放在同一个测试里顺序执行
#[test]
fn anthropic_streaming() {
    std::env::set_var("ANTHROPIC_API_KEY", "test-key");
    let settings = AiSettings::default(); // claude-opus-5-5

    // 1. 正常回答；请求头与请求体
    let (addr, rx) = serve(
        "200 OK",
        sse(&[
            r#"{"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":12}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"你好"}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"，世界"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7}}"#,
            r#"{"type":"message_stop"}"#,
        ]),
    );
    std::env::set_var("ANTHROPIC_BASE_URL", &addr);
    let (r, events) = run(&settings);
    let out = r.unwrap();
    assert_eq!(out.text, "你好，世界");
    assert_eq!(out.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!((out.input_tokens, out.output_tokens), (12, 7));
    assert_eq!(events.len(), 2);
    let sent = rx.recv().unwrap();
    assert!(sent.starts_with("POST /v1/messages"));
    assert!(sent.contains("x-api-key: test-key"));
    assert!(sent.contains("anthropic-version: 2023-06-01"));
    assert!(sent.contains("anthropic-beta: server-side-fallback-2026-07-01"));
    let body: serde_json::Value =
        serde_json::from_str(sent.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["model"], "claude-opus-5-5");
    assert_eq!(body["stream"], true);
    assert_eq!(body["fallbacks"], "default");
    assert_eq!(body["output_config"]["effort"], "medium");
    assert!(
        body.get("thinking").is_none(),
        "Opus 5.5 不能显式关闭思考，省略即可"
    );

    // 2. 服务端回退：原模型拒绝后由回退模型继续
    let (addr, _rx) = serve(
        "200 OK",
        sse(&[
            r#"{"type":"message_start","message":{"model":"claude-opus-4-8","usage":{"input_tokens":5}}}"#,
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"fallback","from":{"model":"claude-opus-5-5"},"to":{"model":"claude-opus-4-8"}}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"OK"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#,
        ]),
    );
    std::env::set_var("ANTHROPIC_BASE_URL", &addr);
    let (r, events) = run(&settings);
    let out = r.unwrap();
    assert_eq!(out.model, "claude-opus-4-8");
    assert!(events
        .iter()
        .any(|e| matches!(e, StreamEvent::Notice(n) if n.contains("claude-opus-4-8"))));

    // 3. 仍被拒绝：返回错误，不把部分输出当成结果
    let (addr, _rx) = serve(
        "200 OK",
        sse(&[
            r#"{"type":"message_start","message":{"model":"claude-opus-5-5","usage":{"input_tokens":5}}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"partial"}}"#,
            r#"{"type":"message_delta","delta":{"stop_reason":"refusal","stop_details":{"type":"refusal","category":"cyber"}},"usage":{"output_tokens":1}}"#,
        ]),
    );
    std::env::set_var("ANTHROPIC_BASE_URL", &addr);
    let (r, _) = run(&settings);
    assert!(
        matches!(r, Err(AiError::Refusal(ref c)) if c.contains("cyber")),
        "{r:?}"
    );

    // 4. HTTP 错误：取出服务商的错误信息
    let (addr, _rx) = serve(
        "401 Unauthorized",
        r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#
            .into(),
    );
    std::env::set_var("ANTHROPIC_BASE_URL", &addr);
    let (r, _) = run(&settings);
    assert!(
        matches!(r, Err(AiError::Http { status: 401, ref message }) if message == "invalid x-api-key"),
        "{r:?}"
    );

    // 5. Haiku 不支持服务端回退：不带 fallbacks
    let (addr, rx) = serve(
        "200 OK",
        sse(&[r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"}}"#]),
    );
    std::env::set_var("ANTHROPIC_BASE_URL", &addr);
    let haiku = AiSettings {
        anthropic_model: "claude-haiku-5-5".into(),
        ..AiSettings::default()
    };
    run(&haiku).0.unwrap();
    let sent = rx.recv().unwrap();
    assert!(!sent.contains("anthropic-beta"));
    assert!(!sent.contains("\"fallbacks\""));
}

#[test]
fn openai_compatible_streaming() {
    let body = [
        r#"data: {"choices":[{"delta":{"role":"assistant","content":""}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#,
        r#"data: {"choices":[{"delta":{"content":"lo"},"finish_reason":"stop"}]}"#,
        "data: [DONE]",
    ]
    .map(|l| format!("{l}\n\n"))
    .concat();
    let (addr, rx) = serve("200 OK", body);
    let settings = AiSettings {
        provider: ProviderKind::OpenaiCompatible,
        openai_base_url: format!("{addr}/v1"),
        openai_model: "qwen2.5:7b".into(),
        ..AiSettings::default()
    };
    let (r, _) = run(&settings);
    let out = r.unwrap();
    assert_eq!(out.text, "Hello");
    assert_eq!(out.stop_reason.as_deref(), Some("stop"));
    let sent = rx.recv().unwrap();
    assert!(sent.starts_with("POST /v1/chat/completions"));
    let body: serde_json::Value =
        serde_json::from_str(sent.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["content"], "hi");
}
