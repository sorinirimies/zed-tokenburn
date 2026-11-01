//! End-to-end: the real binary over stdio, on a fake home with one pi session, so
//! nothing on the developer's machine is read or written.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const BIN: &str = env!("CARGO_BIN_EXE_tokenburn-zed");

/// A fake home with one pi turn 10 minutes ago: 1000 in, 500 out, $0.50.
fn fake_home() -> tempfile::TempDir {
    let home = tempfile::tempdir().unwrap();
    let sessions = home.path().join("sessions").join("--tmp-atlas--");
    std::fs::create_dir_all(&sessions).unwrap();
    let ts = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
    let line = json!({
        "timestamp": ts,
        "message": { "usage": { "input": 1000, "output": 500, "cacheRead": 0, "cacheWrite": 0,
                                 "cost": { "total": 0.5 } } }
    });
    std::fs::write(sessions.join("s1.jsonl"), format!("{line}\n")).unwrap();
    home
}

fn command(home: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(BIN);
    c.args(args);
    let none = home.join("none");
    for (k, v) in [
        ("HOME", home.to_path_buf()),
        ("USERPROFILE", home.to_path_buf()),
        ("XDG_CONFIG_HOME", home.join("config")),
        ("XDG_DATA_HOME", home.join("data")),
        ("XDG_CACHE_HOME", home.join("cache")),
        ("APPDATA", home.join("appdata")),
        ("LOCALAPPDATA", home.join("localappdata")),
        ("TOKENBURN_CONFIG_DIR", home.join("tokenburn")),
        ("TOKENBURN_PI_SESSIONS", home.join("sessions")),
        ("TOKENBURN_ZED_DB", none.join("threads.db")),
        ("TOKENBURN_RTK_DB", none.join("history.db")),
        ("CLAUDE_CONFIG_DIR", none.clone()),
        ("CODEX_HOME", none.clone()),
        ("COPILOT_HOME", none.clone()),
        ("GEMINI_DATA_DIR", none.clone()),
        ("OPENCODE_DATA_DIR", none.clone()),
        ("AMP_DATA_DIR", none.clone()),
        ("CLINE_DATA_DIR", none.clone()),
        ("ROO_DATA_DIR", none.clone()),
        ("KILO_DATA_DIR", none.clone()),
        ("QWEN_DATA_DIR", none.clone()),
    ] {
        c.env(k, v);
    }
    c.env_remove("TOKENBURN_ZED_SETTINGS");
    c
}

#[test]
fn status_prints_the_status_bar_line() {
    let home = fake_home();
    let out = command(home.path(), &["status", "--tool", "pi"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(
        s.starts_with("TokenBurn: 🔥 1.5k ($0.50) [today · pi]"),
        "{s}"
    );
}

#[test]
fn report_prints_markdown() {
    let home = fake_home();
    let out = command(home.path(), &["report"]).output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8(out.stdout).unwrap();
    assert!(s.starts_with("# 🔥 TokenBurn"));
    assert!(s.contains("| today | 1.5k |"), "{s}");
}

#[test]
fn bad_arguments_fail_with_a_message() {
    let home = fake_home();
    let out = command(home.path(), &["status", "--period", "decade"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("period must be"));
}

struct Lsp {
    child: Child,
    out: BufReader<ChildStdout>,
}

impl Lsp {
    fn send(&mut self, v: Value) {
        let body = v.to_string();
        let stdin = self.child.stdin.as_mut().unwrap();
        write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        stdin.flush().unwrap();
    }

    fn recv(&mut self) -> Value {
        let mut len = 0;
        loop {
            let mut line = String::new();
            self.out.read_line(&mut line).unwrap();
            let line = line.trim();
            if line.is_empty() {
                break;
            }
            if let Some(n) = line.strip_prefix("Content-Length:") {
                len = n.trim().parse().unwrap();
            }
        }
        let mut buf = vec![0; len];
        self.out.read_exact(&mut buf).unwrap();
        serde_json::from_slice(&buf).unwrap()
    }
}

#[test]
fn the_language_server_shows_the_line_as_progress() {
    let home = fake_home();
    let mut child = command(home.path(), &["lsp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let out = BufReader::new(child.stdout.take().unwrap());
    let mut lsp = Lsp { child, out };
    lsp.send(
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "capabilities": { "window": { "workDoneProgress": true } },
        "initializationOptions": { "tool": "pi", "show_rate": false } } }),
    );
    assert_eq!(lsp.recv()["result"]["serverInfo"]["name"], "tokenburn-zed");
    lsp.send(json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut message = None;
    while message.is_none() && Instant::now() < deadline {
        let m = lsp.recv();
        match m["method"].as_str() {
            Some("window/workDoneProgress/create") => {
                lsp.send(json!({ "jsonrpc": "2.0", "id": m["id"], "result": null }))
            }
            Some("$/progress") if m["params"]["value"]["kind"] == "begin" => {
                assert_eq!(m["params"]["value"]["title"], "TokenBurn");
                message = m["params"]["value"]["message"].as_str().map(String::from);
            }
            _ => {}
        }
    }
    let message = message.expect("a progress begin");
    assert!(
        message.starts_with("🔥 1.5k ($0.50) [today · pi]"),
        "{message}"
    );

    lsp.send(json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }));
    loop {
        let m = lsp.recv();
        if m["id"] == 2 {
            break;
        }
    }
    lsp.send(json!({ "jsonrpc": "2.0", "method": "exit" }));
    let status = lsp.child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn the_mcp_server_answers_tool_calls() {
    let home = fake_home();
    let mut child = command(home.path(), &["mcp"])
        .env("TOKENBURN_ZED_SETTINGS", r#"{"tool":"pi"}"#)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for m in [
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                    "params": { "name": "tokenburn_usage", "arguments": { "period": "today" } } }),
        ] {
            writeln!(stdin, "{m}").unwrap();
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    let lines: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 2);
    let text = lines[1]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("today (pi"), "{text}");
    assert!(text.contains("- turns: 1"), "{text}");
    assert!(text.contains("- cost: $0.50"), "{text}");
}

#[test]
fn without_a_mode_the_protocol_is_detected() {
    let home = fake_home();
    // MCP: a JSON line.
    let mut child = command(home.path(), &[])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        child.stdin.as_mut().unwrap(),
        "{}",
        json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" })
    )
    .unwrap();
    drop(child.stdin.take());
    let out = child.wait_with_output().unwrap();
    let reply: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(reply["id"], 7);

    // LSP: a Content-Length frame.
    let mut child = command(home.path(), &[])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = BufReader::new(child.stdout.take().unwrap());
    let mut lsp = Lsp { child, out };
    lsp.send(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "capabilities": {} } }));
    assert_eq!(lsp.recv()["result"]["serverInfo"]["name"], "tokenburn-zed");
    lsp.send(json!({ "jsonrpc": "2.0", "method": "initialized", "params": {} }));
    lsp.send(json!({ "jsonrpc": "2.0", "id": 2, "method": "shutdown" }));
    while lsp.recv()["id"] != 2 {}
    lsp.send(json!({ "jsonrpc": "2.0", "method": "exit" }));
    assert!(lsp.child.wait().unwrap().success());
}
