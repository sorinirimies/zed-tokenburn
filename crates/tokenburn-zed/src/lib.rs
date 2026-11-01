//! # tokenburn-zed
//!
//! The native half of the [TokenBurn](https://github.com/sorinirimies/tokenburn)
//! Zed extension. One binary, three modes:
//!
//! * `tokenburn-zed lsp` (default) — a language server whose only job is the
//!   status bar: it keeps one work-done progress open and Zed shows its message at
//!   the bottom of the window (`TokenBurn: 🔥 1.20M ($5.10) [today] · …`). Budget
//!   alerts become Zed notifications; code actions open the full report, cycle the
//!   period and pause.
//! * `tokenburn-zed mcp` — an MCP server (the extension's context server) with
//!   read-only tools for Zed's agent: usage, report, chart, breakdown, budgets.
//! * `tokenburn-zed status|report` — the same numbers on the terminal.
//!
//! All log reading and aggregation is `tokenburn-core`'s: pi, Zed, Claude Code,
//! Codex, Copilot CLI, Gemini CLI, OpenCode, Amp, Cline, Roo Code, Kilo Code and
//! Qwen Code, from local files only — no network.

pub mod budget;
pub mod chart;
pub mod config;
pub mod engine;
pub mod fmt;
pub mod lsp;
pub mod mcp;
pub mod report;
pub mod span;
pub mod stats;
pub mod status;

use std::path::PathBuf;

use chrono::{DateTime, Local, Utc};

use crate::config::Config;
use crate::span::Span;

/// The full markdown report with every tool's history, as of now.
pub fn report_text(cfg: &Config, breakdown: Span) -> String {
    let now = Local::now();
    let data = stats::load(DateTime::<Utc>::UNIX_EPOCH);
    report::full(&report::Context::new(&data, cfg, now), breakdown)
}

/// Where the code action writes the report: `<cache dir>/tokenburn/zed-report.md`.
pub fn report_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("tokenburn")
        .join("zed-report.md")
}

/// Write the report to [`report_path`] and return the path.
pub fn write_report(cfg: &Config) -> Result<PathBuf, String> {
    let path = report_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(&path, report_text(cfg, Span::Month))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}
