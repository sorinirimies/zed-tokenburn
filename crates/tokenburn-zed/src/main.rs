//! `tokenburn-zed` — see the crate docs. `lsp` is the default mode, so Zed can
//! start the binary with no arguments.

use std::io::{BufRead, BufReader};
use std::process::ExitCode;

use serde_json::{json, Map, Value};
use tokenburn_zed::config::Config;
use tokenburn_zed::span::Span;
use tokenburn_zed::{engine, lsp, mcp};

const HELP: &str = "\
tokenburn-zed — AI token and cost usage for Zed (TokenBurn)

USAGE:
    tokenburn-zed lsp                   language server (status bar), what Zed starts
    tokenburn-zed mcp                   MCP server with read-only usage tools
    tokenburn-zed                       either: picks LSP or MCP from the first message
    tokenburn-zed status [OPTIONS]      print the status-bar line once
    tokenburn-zed report [OPTIONS]      print the full markdown report
    tokenburn-zed dashboard [gui|web] [OPTIONS]
                                        open tokenburn's desktop app or web dashboard
                                        (tokenburn-gui / tokenburn-web) on that view

OPTIONS (status, report, dashboard):
    --period <hour|today|week|month|year|all>
    --tool <all|pi|zed|claude|codex|copilot|gemini|opencode|amp|cline|roo|kilo|qwen>
    --no-cache                          count input + output tokens only

    -h, --help       this help
    -V, --version    version

Settings (JSON, like Zed's `lsp.tokenburn.settings`) are also read from
$TOKENBURN_ZED_SETTINGS.";

/// Turn `--period week --tool pi --no-cache` into a settings layer.
fn flags(args: &[String]) -> Result<Value, String> {
    let mut m = Map::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--period" | "--tool" => {
                let v = it.next().ok_or(format!("{a} needs a value"))?;
                let key = &a[2..];
                if key == "period" {
                    v.parse::<Span>()?;
                } else {
                    v.parse::<tokenburn_core::ToolFilter>()?;
                }
                m.insert(key.into(), json!(v));
            }
            "--no-cache" => {
                m.insert("include_cache".into(), json!(false));
            }
            other => return Err(format!("unknown option {other:?} (see --help)")),
        }
    }
    Ok(Value::Object(m))
}

fn config(args: &[String]) -> Result<Config, String> {
    let mut cfg = Config::from_env();
    cfg.apply(&flags(args)?);
    cfg.enabled = true;
    Ok(cfg)
}

/// Which protocol a client speaks, from the first bytes it sent: MCP stdio is
/// bare JSON (`{` or a `[` batch), LSP starts with a `Content-Length` header.
#[derive(Debug, PartialEq, Eq)]
enum Protocol {
    Lsp,
    Mcp,
}

fn sniff(input: &mut impl BufRead) -> std::io::Result<Protocol> {
    loop {
        let buf = input.fill_buf()?;
        let Some(&first) = buf.first() else {
            return Ok(Protocol::Lsp); // EOF: nothing to serve either way
        };
        if first.is_ascii_whitespace() {
            input.consume(1);
            continue;
        }
        return Ok(if first == b'{' || first == b'[' {
            Protocol::Mcp
        } else {
            Protocol::Lsp
        });
    }
}

fn stdin() -> BufReader<std::io::Stdin> {
    BufReader::new(std::io::stdin())
}

fn run(args: &[String]) -> Result<(), String> {
    let Some((mode, rest)) = args.split_first() else {
        // No mode: a misconfigured client still gets the right server.
        let mut input = stdin();
        return match sniff(&mut input).map_err(|e| e.to_string())? {
            Protocol::Lsp => lsp::run(input).map_err(|e| e.to_string()),
            Protocol::Mcp => mcp::run(input).map_err(|e| e.to_string()),
        };
    };
    match mode.as_str() {
        "lsp" | "--stdio" => lsp::run(stdin()).map_err(|e| e.to_string()),
        "mcp" => mcp::run(stdin()).map_err(|e| e.to_string()),
        "status" => {
            let cfg = config(rest)?;
            let snap = engine::refresh(&cfg);
            println!("{}: {}", lsp::TITLE, snap.status.unwrap_or_default());
            for w in snap.warnings {
                eprintln!("warning: {w}");
            }
            Ok(())
        }
        "dashboard" => {
            let (kind, rest) = match rest.split_first() {
                Some((k, r)) if k == "gui" || k == "web" => (k.as_str(), r),
                _ => ("gui", rest),
            };
            let cfg = config(rest)?;
            if kind == "gui" {
                let path = tokenburn_zed::dashboard::open_gui(&cfg)?;
                println!("started {}", path.display());
            } else {
                let (url, _child) =
                    tokenburn_zed::dashboard::ensure_web(&cfg, std::time::Duration::from_secs(15))?;
                println!("{url}");
            }
            Ok(())
        }
        "report" => {
            let cfg = config(rest)?;
            let span = cfg.period;
            print!("{}", tokenburn_zed::report_text(&cfg, span));
            Ok(())
        }
        "-h" | "--help" | "help" => {
            println!("{HELP}");
            Ok(())
        }
        "-V" | "--version" => {
            println!("tokenburn-zed {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => Err(format!("unknown mode {other:?} (see --help)")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("tokenburn-zed: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn flags_become_a_settings_layer() {
        let v = flags(&s(&["--period", "week", "--tool", "pi", "--no-cache"])).unwrap();
        assert_eq!(
            v,
            json!({ "period": "week", "tool": "pi", "include_cache": false })
        );
        assert_eq!(flags(&[]).unwrap(), json!({}));
    }

    #[test]
    fn bad_flags_are_errors() {
        assert!(flags(&s(&["--period"])).is_err());
        assert!(flags(&s(&["--period", "decade"])).is_err());
        assert!(flags(&s(&["--tool", "notepad"])).is_err());
        assert!(flags(&s(&["--verbose"])).is_err());
    }

    #[test]
    fn sniff_tells_mcp_from_lsp() {
        let mut mcp = std::io::Cursor::new(b"\n  {\"jsonrpc\":\"2.0\"}\n".to_vec());
        assert_eq!(sniff(&mut mcp).unwrap(), Protocol::Mcp);
        let mut rest = String::new();
        mcp.read_line(&mut rest).unwrap();
        assert!(
            rest.starts_with('{'),
            "only the whitespace is consumed: {rest:?}"
        );
        let mut batch = std::io::Cursor::new(b"[{}]".to_vec());
        assert_eq!(sniff(&mut batch).unwrap(), Protocol::Mcp);
        let mut lsp = std::io::Cursor::new(b"Content-Length: 2\r\n\r\n{}".to_vec());
        assert_eq!(sniff(&mut lsp).unwrap(), Protocol::Lsp);
        let mut empty = std::io::Cursor::new(Vec::new());
        assert_eq!(sniff(&mut empty).unwrap(), Protocol::Lsp);
    }

    #[test]
    fn unknown_modes_are_errors_and_help_works() {
        assert!(run(&s(&["serve"])).is_err());
        assert!(run(&s(&["dashboard", "web", "--period", "decade"])).is_err());
        assert!(run(&s(&["--help"])).is_ok());
        assert!(run(&s(&["--version"])).is_ok());
    }
}
