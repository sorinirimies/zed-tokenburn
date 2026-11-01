//! `tokenburn-zed` — see the crate docs. `lsp` is the default mode, so Zed can
//! start the binary with no arguments.

use std::process::ExitCode;

use serde_json::{json, Map, Value};
use tokenburn_zed::config::Config;
use tokenburn_zed::span::Span;
use tokenburn_zed::{engine, lsp, mcp};

const HELP: &str = "\
tokenburn-zed — AI token and cost usage for Zed (TokenBurn)

USAGE:
    tokenburn-zed [lsp]                 language server (status bar), what Zed starts
    tokenburn-zed mcp                   MCP server with read-only usage tools
    tokenburn-zed status [OPTIONS]      print the status-bar line once
    tokenburn-zed report [OPTIONS]      print the full markdown report

OPTIONS (status, report):
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

fn run(args: &[String]) -> Result<(), String> {
    let (mode, rest) = match args.split_first() {
        Some((m, rest)) => (m.as_str(), rest),
        None => ("lsp", &[][..]),
    };
    match mode {
        "lsp" | "--stdio" => lsp::run().map_err(|e| e.to_string()),
        "mcp" => mcp::run().map_err(|e| e.to_string()),
        "status" => {
            let cfg = config(rest)?;
            let snap = engine::refresh(&cfg);
            println!("{}: {}", lsp::TITLE, snap.status.unwrap_or_default());
            for w in snap.warnings {
                eprintln!("warning: {w}");
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
    fn unknown_modes_are_errors_and_help_works() {
        assert!(run(&s(&["serve"])).is_err());
        assert!(run(&s(&["--help"])).is_ok());
        assert!(run(&s(&["--version"])).is_ok());
    }
}
