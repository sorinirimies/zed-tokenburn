//! A small MCP server (`tokenburn-zed mcp`), registered as the extension's context
//! server, so Zed's agent can answer "what did I spend today?" itself.
//!
//! Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout (MCP stdio). Only the
//! parts of the protocol a tools-only server needs: `initialize`, `ping`,
//! `tools/list`, `tools/call`. Every tool is read-only and local.

use std::io::{BufRead, Write};
use std::str::FromStr;

use chrono::{DateTime, Local, Utc};
use serde_json::{json, Value};
use tokenburn_core::ToolFilter;

use crate::chart::{self, Metric, View};
use crate::config::Config;
use crate::report::{self, Context, Group};
use crate::span::Span;
use crate::stats::Data;

/// The protocol revision answered when the client asks for one we don't know.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
const KNOWN_VERSIONS: [&str; 4] = ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"];

/// Reads the logs from a start instant (the real one is [`crate::stats::load`]).
pub type Loader<'a> = &'a dyn Fn(DateTime<Utc>) -> Data;

fn tool_prop() -> Value {
    let mut names = vec!["all".to_string()];
    names.extend(
        tokenburn_core::Tool::ALL
            .iter()
            .map(|t| t.as_str().to_string()),
    );
    json!({ "type": "string", "enum": names,
        "description": "Only count this tool (default: the configured one, usually all)" })
}

fn period_prop(default: &str) -> Value {
    json!({ "type": "string", "enum": ["hour", "today", "week", "month", "year", "all"],
        "description": format!("Time span, local time, weeks start Monday (default {default})") })
}

/// `tools/list`.
pub fn tools() -> Value {
    json!({ "tools": [
        {
            "name": "tokenburn_usage",
            "title": "Token usage",
            "description": "AI coding-agent token usage and cost for a period, read from the local logs of pi, Zed, Claude Code, Codex, Copilot CLI, Gemini CLI, OpenCode, Amp, Cline, Roo Code, Kilo Code and Qwen Code: tokens (input/output/cache), turns, cost, burn rate, cache hit ratio, estimated savings.",
            "inputSchema": { "type": "object", "properties": {
                "period": period_prop("today"), "tool": tool_prop() } },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "tokenburn_report",
            "title": "Full usage report",
            "description": "The full TokenBurn report as markdown: usage for every period, budgets with forecasts, daily/weekly/monthly charts, usage by tool and by project, savings.",
            "inputSchema": { "type": "object", "properties": {
                "tool": tool_prop(), "breakdown_period": period_prop("month") } },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "tokenburn_chart",
            "title": "Usage chart",
            "description": "A text column chart of tokens or cost: the last 14 days, 8 weeks or 6 months.",
            "inputSchema": { "type": "object", "properties": {
                "view": { "type": "string", "enum": ["day", "week", "month"], "description": "Column size (default day)" },
                "metric": { "type": "string", "enum": ["tokens", "cost"], "description": "What to plot (default tokens)" },
                "tool": tool_prop() } },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "tokenburn_breakdown",
            "title": "Usage breakdown",
            "description": "Usage grouped by tool or by project (the model for tools that log no project), biggest first, with tokens, cache hit ratio, cost and share.",
            "inputSchema": { "type": "object", "properties": {
                "by": { "type": "string", "enum": ["tool", "project"], "description": "Grouping (default tool)" },
                "period": period_prop("month"), "tool": tool_prop(),
                "top": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Rows before the rest is summed up (default 10)" } } },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
        {
            "name": "tokenburn_budgets",
            "title": "Budgets and limits",
            "description": "Daily/weekly/monthly token budgets and spending limits: how much is used and when the current pace runs out.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true, "openWorldHint": false },
        },
    ]})
}

fn arg<T: FromStr<Err = String>>(args: &Value, key: &str) -> Result<Option<T>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => s.parse().map(Some),
        Some(other) => Err(format!("{key} must be a string (got {other})")),
    }
}

fn text_result(text: String, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

/// Run one tool. `Err` is a tool error (reported to the model, not a protocol error).
pub fn call_tool(
    name: &str,
    args: &Value,
    base: &Config,
    load: Loader,
    now: DateTime<Local>,
) -> Result<String, String> {
    let mut cfg = base.clone();
    if let Some(t) = arg::<ToolFilter>(args, "tool")? {
        cfg.tool = t;
    }
    match name {
        "tokenburn_usage" => {
            let span = arg::<Span>(args, "period")?.unwrap_or(Span::Today);
            let data = load(span.start(now));
            Ok(report::usage_text(&Context::new(&data, &cfg, now), span))
        }
        "tokenburn_report" => {
            let span = arg::<Span>(args, "breakdown_period")?.unwrap_or(Span::Month);
            let data = load(DateTime::<Utc>::UNIX_EPOCH);
            Ok(report::full(&Context::new(&data, &cfg, now), span))
        }
        "tokenburn_chart" => {
            let view = arg::<View>(args, "view")?.unwrap_or_default();
            let metric = arg::<Metric>(args, "metric")?.unwrap_or_default();
            let data = load(chart::chart_start(view, now));
            Ok(report::chart_block(
                &Context::new(&data, &cfg, now),
                view,
                metric,
            ))
        }
        "tokenburn_breakdown" => {
            let group = arg::<Group>(args, "by")?.unwrap_or_default();
            let span = arg::<Span>(args, "period")?.unwrap_or(Span::Month);
            let top = match args.get("top") {
                None | Some(Value::Null) => 10,
                Some(v) => {
                    v.as_u64()
                        .filter(|n| (1..=100).contains(n))
                        .ok_or("top must be an integer from 1 to 100")? as usize
                }
            };
            let data = load(span.start(now));
            let ctx = Context::new(&data, &cfg, now);
            let entries = report::breakdown(&data, cfg.tool, span, group, cfg.include_cache, now);
            Ok(format!(
                "Usage by {} — {}\n\n{}",
                match group {
                    Group::Tool => "tool",
                    Group::Project => "project",
                },
                span.title(),
                report::breakdown_table(&entries, top, group, &ctx.cur)
            ))
        }
        "tokenburn_budgets" => {
            let data = load(Span::Month.start(now).min(Span::Week.start(now)));
            Ok(report::budgets_text(&Context::new(&data, &cfg, now)))
        }
        other => Err(format!("unknown tool {other:?}")),
    }
}

/// Answer one JSON-RPC message; `None` for notifications.
pub fn handle(msg: &Value, cfg: &Config, load: Loader, now: DateTime<Local>) -> Option<Value> {
    let id = msg.get("id")?.clone();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let ok = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    let err = |code: i64, message: String| json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } });
    Some(match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(PROTOCOL_VERSION);
            let version = if KNOWN_VERSIONS.contains(&asked) {
                asked
            } else {
                PROTOCOL_VERSION
            };
            ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "tokenburn", "title": "TokenBurn", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Local, read-only AI token usage and cost from the logs of pi, Zed, Claude Code, Codex and other coding agents. Costs are what the tools logged (Zed logs none).",
            }))
        }
        "ping" => ok(json!({})),
        "tools/list" => ok(tools()),
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(err(-32602, "tools/call needs a name".into()));
            };
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match call_tool(name, &args, cfg, load, now) {
                Ok(text) => ok(text_result(text, false)),
                Err(e) if e.starts_with("unknown tool") => err(-32602, e),
                Err(e) => ok(text_result(e, true)),
            }
        }
        "resources/list" => ok(json!({ "resources": [] })),
        "prompts/list" => ok(json!({ "prompts": [] })),
        other => err(-32601, format!("method not found: {other}")),
    })
}

/// Serve MCP over `input`/`output` until the input ends.
pub fn serve(
    input: impl BufRead,
    mut output: impl Write,
    cfg: &Config,
    load: Loader,
) -> std::io::Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let out: Vec<Value> = batch
                    .iter()
                    .filter_map(|m| handle(m, cfg, load, Local::now()))
                    .collect();
                (!out.is_empty()).then_some(Value::Array(out))
            }
            Ok(msg) => handle(&msg, cfg, load, Local::now()),
            Err(e) => Some(json!({ "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("parse error: {e}") } })),
        };
        if let Some(r) = reply {
            serde_json::to_writer(&mut output, &r)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
    Ok(())
}

/// The real thing, reading `input` and writing stdout.
pub fn run(input: impl BufRead) -> std::io::Result<()> {
    let cfg = Config::from_env();
    serve(input, std::io::stdout(), &cfg, &crate::stats::load)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::tests::{now, row};
    use chrono::Duration;
    use tokenburn_core::Tool;

    fn fake(_: DateTime<Utc>) -> Data {
        let n = now();
        Data {
            rows: vec![
                row(
                    Tool::Pi,
                    "/Users/me/atlas",
                    n - Duration::hours(1),
                    1000,
                    2.0,
                ),
                row(
                    Tool::Claude,
                    "/Users/me/orbit",
                    n - Duration::days(1),
                    500,
                    1.0,
                ),
            ],
            ..Default::default()
        }
    }

    fn call(name: &str, args: Value) -> Result<String, String> {
        call_tool(name, &args, &Config::default(), &fake, now())
    }

    #[test]
    fn initialize_negotiates_the_version() {
        let r = handle(
            &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                     "params": { "protocolVersion": "2024-11-05" } }),
            &Config::default(),
            &fake,
            now(),
        )
        .unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let r = handle(
            &json!({ "id": 2, "method": "initialize", "params": { "protocolVersion": "1999-01-01" } }),
            &Config::default(), &fake, now(),
        )
        .unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_methods_an_error() {
        let cfg = Config::default();
        assert!(handle(
            &json!({ "method": "notifications/initialized" }),
            &cfg,
            &fake,
            now()
        )
        .is_none());
        let r = handle(
            &json!({ "id": 3, "method": "sampling/whatever" }),
            &cfg,
            &fake,
            now(),
        )
        .unwrap();
        assert_eq!(r["error"]["code"], -32601);
        let r = handle(&json!({ "id": 4, "method": "ping" }), &cfg, &fake, now()).unwrap();
        assert_eq!(r["result"], json!({}));
    }

    #[test]
    fn every_listed_tool_runs() {
        let list = tools();
        let names: Vec<&str> = list["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert_eq!(names.len(), 5);
        for n in names {
            let out = call(n, json!({})).unwrap_or_else(|e| panic!("{n}: {e}"));
            assert!(!out.is_empty(), "{n}");
        }
    }

    #[test]
    fn tools_honour_their_arguments() {
        let usage = call(
            "tokenburn_usage",
            json!({ "period": "week", "tool": "claude" }),
        )
        .unwrap();
        assert!(usage.contains("this week (Claude Code"), "{usage}");
        assert!(usage.contains("- turns: 1"));
        let chart = call(
            "tokenburn_chart",
            json!({ "view": "month", "metric": "cost" }),
        )
        .unwrap();
        assert!(chart.contains("Monthly cost · last 6 months"));
        let by = call(
            "tokenburn_breakdown",
            json!({ "by": "project", "period": "all", "top": 1 }),
        )
        .unwrap();
        assert!(
            by.contains("| pi:atlas |") && by.contains("| 1 more |"),
            "{by}"
        );
    }

    #[test]
    fn bad_arguments_are_tool_errors() {
        assert!(call("tokenburn_usage", json!({ "period": "decade" })).is_err());
        assert!(call("tokenburn_usage", json!({ "tool": "notepad" })).is_err());
        assert!(call("tokenburn_usage", json!({ "period": 7 })).is_err());
        assert!(call("tokenburn_breakdown", json!({ "top": 0 })).is_err());
        let r = handle(
            &json!({ "id": 5, "method": "tools/call", "params": { "name": "tokenburn_usage", "arguments": { "period": "x" } } }),
            &Config::default(), &fake, now(),
        )
        .unwrap();
        assert_eq!(r["result"]["isError"], true);
        let r = handle(
            &json!({ "id": 6, "method": "tools/call", "params": { "name": "nope" } }),
            &Config::default(),
            &fake,
            now(),
        )
        .unwrap();
        assert_eq!(r["error"]["code"], -32602);
    }

    #[test]
    fn serve_speaks_newline_delimited_json() {
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            "\n",
            "not json\n",
            r#"[{"jsonrpc":"2.0","id":2,"method":"ping"},{"jsonrpc":"2.0","method":"x"}]"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
            "\n",
        );
        let mut out = Vec::new();
        serve(input.as_bytes(), &mut out, &Config::default(), &fake).unwrap();
        let lines: Vec<Value> = String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 4, "{lines:?}");
        assert_eq!(lines[0]["id"], 1);
        assert_eq!(lines[1]["error"]["code"], -32700);
        assert_eq!(
            lines[2].as_array().unwrap().len(),
            1,
            "notification in a batch: no reply"
        );
        assert_eq!(lines[3]["result"]["tools"].as_array().unwrap().len(), 5);
    }
}
