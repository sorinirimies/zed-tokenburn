<div align="center">

# 🔥 TokenBurn for Zed

**Live AI token and cost usage at the bottom of your Zed window — from the logs already on your disk.**

All Rust. Local logs only: no network calls, no account, no telemetry.

[![CI](https://github.com/sorinirimies/zed-tokenburn/actions/workflows/ci.yml/badge.svg)](https://github.com/sorinirimies/zed-tokenburn/actions/workflows/ci.yml)
[![Security](https://github.com/sorinirimies/zed-tokenburn/actions/workflows/security.yml/badge.svg)](https://github.com/sorinirimies/zed-tokenburn/actions/workflows/security.yml)
[![Fuzz](https://github.com/sorinirimies/zed-tokenburn/actions/workflows/fuzz.yml/badge.svg)](https://github.com/sorinirimies/zed-tokenburn/actions/workflows/fuzz.yml)
[![Crates.io](https://img.shields.io/crates/v/tokenburn-zed.svg)](https://crates.io/crates/tokenburn-zed)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

```text
TokenBurn: 🔥 51.50M ($20.04) [today] · 51.50M/h · 97% hit · ~$135.38 saved
```

The Zed member of the [TokenBurn](https://github.com/sorinirimies/tokenburn) family
(CLI / TUI / GUI / web) and the sibling of [pi-tokenburn](https://github.com/sorinirimies/pi-tokenburn):
the same numbers, the same budgets and alerts, now in Zed's status bar. It reads
**pi, Zed's own agent panel, Claude Code, Codex, Copilot CLI, Gemini CLI, OpenCode,
Amp, Cline, Roo Code, Kilo Code and Qwen Code** through
[`tokenburn-core`](https://crates.io/crates/tokenburn-core).

## Features

| | |
|---|---|
| **Status bar** | `🔥 tokens (cost) [period] · burn rate · cache hit · ~saved`, refreshed every 10 s, only re-parsing files that changed. Turns into `🔥 ⚠ …` near a budget and `🔥 OVER BUDGET …` past it |
| **Periods** | last hour, today, this week (Monday start), month, year, all time — set it in the settings or cycle it with a code action |
| **Budgets & limits** | daily / weekly / monthly **token** budgets (Zed settings) and **dollar** limits (shared with every TokenBurn front-end), each alerting once per period as a Zed notification, with a linear forecast ("at this pace it runs out Thu 14:05") |
| **Dashboards** | code actions open tokenburn's own **Rust dashboards** on the status bar's view — the native Iced desktop app (`tokenburn-gui`) or the live web dashboard (`tokenburn-web`, opened in your browser): interactive animated charts, click-to-drill-down, 43 themes, limits and savings panels |
| **Report** | a code action opens a markdown report in a tab: usage for every period, budgets, daily / weekly / monthly charts, usage by tool and by project, savings by source |
| **Savings** | prompt cache, [rtk](https://github.com/rtk-ai/rtk) and pi compaction, valued at your input price |
| **Agent tools (MCP)** | Zed's agent can answer "what did I spend this week?" itself: `tokenburn_usage`, `tokenburn_report`, `tokenburn_chart`, `tokenburn_breakdown`, `tokenburn_budgets` |
| **Filters** | one tool or all, with or without cache tokens, any currency TokenBurn knows |

Costs are what the tools log; **Zed, Codex, Gemini, Copilot, Amp and Qwen log none**, so
their cost is `$0.00` rather than a guess (token counts are exact).

## How it works

Zed extensions run as sandboxed WebAssembly and have **no API for status-bar items or
commands**. What Zed *does* show in its status bar is a language server's work-done
progress. So there are two small extensions and one native binary:

```
┌─────────────────── Zed ───────────────────┐        ┌──────── tokenburn-zed (native) ───────┐
│ TokenBurn            (tokenburn, src/)    │ starts │ lsp: one $/progress kept open,        │
│  • language server "tokenburn"            ├───────▶│      message = the status line        │──▶ tokenburn-core
│ TokenBurn MCP Server (tokenburn-mcp-server│        │ mcp: read-only usage tools            │    (local logs)
│  , mcp/) • context server "tokenburn"     ├───────▶│                                       │
└───────────────────────────────────────────┘        └───────────────────────────────────────┘
```

Two extensions because Zed's registry wants an MCP server extension to provide exactly
one context server and nothing else. Install either or both; they share the binary
download code (`src/download.rs`, `src/resolve.rs`, `src/platform.rs`).

* The **language server** never reads or changes your code. It is registered for ~260
  languages only so it runs whatever file is open. It keeps one progress titled
  `TokenBurn` and updates its message; alerts are `window/showMessage`; the code
  actions are `workspace/executeCommand`.
* The **context server** is a small MCP server on stdio.
* Both are the same binary, `tokenburn-zed` (`lsp` / `mcp` / `status` / `report`;
  without a mode it detects LSP or MCP from the first message).

> The status bar shows the activity spinner next to the text (it is a "progress" to
> Zed), and when another language server is busy Zed shows the most recent work
> first. That is the price of having no status-bar API.

## Install

1. **Zed → Extensions** (`zed: extensions`) → search **TokenBurn** → Install (and
   **TokenBurn MCP Server** for the agent tools).
2. Open any file. The extension downloads `tokenburn-zed` for your platform from the
   [latest release](https://github.com/sorinirimies/zed-tokenburn/releases) (macOS
   arm64/x86-64, Linux arm64/x86-64 static, Windows x86-64) and the status bar shows
   `TokenBurn: 🔥 …`. (Zed starts language servers only in projects you trust; it asks
   once per project.)

Prefer your own build? Put it on `PATH` and the status bar uses it instead (for the
MCP server set `binary_path`, below — a context server has no PATH to search):

```sh
cargo install tokenburn-zed
```

From source (dev extensions):

```sh
just dev    # installs tokenburn-zed, builds both wasm modules
# Zed: command palette → "zed: install dev extension" → pick this folder,
# and again → pick ./mcp
```

## Use

**Code actions** — put the cursor on **line 1** of any file and press `cmd-.` / `ctrl-.`:

| Action | |
|---|---|
| 🔥 TokenBurn: open usage report | writes `~/Library/Caches/tokenburn/zed-report.md` (`~/.cache/…` on Linux) and opens it |
| 🔥 TokenBurn: open the desktop dashboard | starts `tokenburn-gui` (shown when it is installed) |
| 🔥 TokenBurn: open the web dashboard | starts `tokenburn-web` on `127.0.0.1:3000` (or reuses a running one) and opens it in the browser; stopped again when Zed stops the server |
| 🔥 TokenBurn: status bar → week (now today) | cycles today → week → month → year → all → hour |
| 🔥 TokenBurn: pause / resume the status bar | paused = nothing read in the background |

They are offered on the first line only because Zed puts a ⚡ button on every line
that has code actions; `"code_actions": "everywhere"` offers them on every line,
`"off"` (or `false`) not at all. Cycling and pausing last until Zed restarts the server
or the settings change; put permanent choices in the settings.

**Agent panel**: install *TokenBurn MCP Server*, enable it (Agent settings → MCP
servers) and ask, e.g. *"Use tokenburn to show my daily token chart"*. It downloads
the binary from the latest release unless `binary_path` points at your own build:

```jsonc
"context_servers": { "tokenburn": { "settings": { "binary_path": "/path/to/tokenburn-zed" } } }
```

Without the MCP extension, the same server works as a plain custom server:

```jsonc
"context_servers": { "tokenburn": { "command": "/path/to/tokenburn-zed", "args": ["mcp"] } }
```

**Dashboards**: Zed extensions can't draw UI, so instead of re-drawing charts the
extension opens the real tokenburn front-ends — same core, same settings, same themes:

```sh
cargo install tokenburn-gui tokenburn-web   # once; found on PATH or in ~/.cargo/bin
```

| Desktop (Iced) | Web (Topcoat, live over a WebSocket) |
|---|---|
| ![tokenburn GUI](https://raw.githubusercontent.com/sorinirimies/tokenburn/main/examples/vhs/generated/gui.gif) | ![tokenburn web](https://raw.githubusercontent.com/sorinirimies/tokenburn/main/examples/vhs/generated/web-interactive.gif) |

Both start on the status bar's tool and period (last hour/today → hourly, week/month →
daily, year/all → monthly). The web dashboard is only reused when it really is
tokenburn-web on that port (checked via `/api/themes`); otherwise set `dashboard.port`.

**Terminal**: `tokenburn-zed status --period week`, `tokenburn-zed report --tool claude`,
`tokenburn-zed dashboard web --period month`.

## Settings

Zed `settings.json` — everything optional:

```jsonc
{
  "lsp": {
    "tokenburn": {
      "settings": {
        "period": "today",          // hour | today | week | month | year | all
        "tool": "all",              // all | pi | zed | claude | codex | copilot | gemini | opencode | amp | cline | roo | kilo | qwen
        "refresh_secs": 10,         // 2–3600
        "include_cache": true,      // count cache read/write tokens
        "show_cost": true,
        "show_rate": true,          // tokens per hour over the period
        "show_hit": true,           // prompt-cache hit ratio
        "show_saved": true,         // estimated savings
        "currency": null,           // "EUR", "GBP", … (default: TokenBurn's shared setting)
        "budgets": {
          "day_tokens": 5000000, "week_tokens": null, "month_tokens": null,
          "day_cost": null, "week_cost": 50, "month_cost": null  // override the shared $ limits
        },
        "warn_pct": null,           // default: the shared limits' warn_at (80 %)
        "notify": true,             // Zed notifications for budget alerts
        "desktop_notifications": false,  // also OS notifications for the $ limits
        "code_actions": "first_line",  // "first_line" | "everywhere" | "off"
        "dashboard": { "port": 3000, "gui_path": null, "web_path": null },
        "enabled": true
      }
      // "binary": { "path": "/path/to/tokenburn-zed", "arguments": ["lsp"] }
    }
  },
  "context_servers": {
    "tokenburn": {
      "settings": {
        "tool": "all"                  // same keys; the defaults of the MCP tools
        // "binary_path": "/path/to/tokenburn-zed"   // MCP extension: skip the download
      }
    }
  }
}
```

**Shared with the other TokenBurn front-ends** (`settings.json` in TokenBurn's config
directory: `~/.config/tokenburn/` on Linux, `~/Library/Application Support/tokenburn/` on
macOS, `%APPDATA%\tokenburn\` on Windows, or `$TOKENBURN_CONFIG_DIR`): display currency and exchange rates, `input_price_per_mtok`
for savings, and the dollar `limits` — set them once with
`tokenburn --limit-daily 20 --limit-weekly 100`, and the CLI watcher, TUI, GUI, web
dashboard and Zed share one alert log, so a limit alerts **once**, not once per app.
Log locations follow tokenburn's environment overrides (`TOKENBURN_PI_SESSIONS`,
`TOKENBURN_ZED_DB`, `CLAUDE_CONFIG_DIR`, `CODEX_HOME`, …).

Want only the counter for Zed's own agent? `"tool": "zed"` (tokens only — Zed logs no cost).

## Development

```sh
just              # list tasks
just check-all    # fmt + clippy (native and wasm32-wasip2) + tests + wasm build + version check + nu tests
just status       # the status line from your real logs
just report       # the full report
just fuzz-all     # every cargo-fuzz target (nightly)
just audit deny   # supply chain
```

Layout:

```
extension.toml, src/           the status-bar extension "tokenburn" (wasm32-wasip2)
  resolve.rs, download.rs,     finding/downloading the binary — pure decisions are unit-tested;
  platform.rs                  shared with the MCP extension via #[path]
mcp/                           the MCP server extension "tokenburn-mcp-server"
  extension.toml, src/, LICENSE → ../LICENSE
  configuration/               its instructions, settings schema, defaults
crates/tokenburn-zed/src/
  span.rs      hour/today/week/month/year/all boundaries (local time, Monday weeks)
  config.rs    lenient settings parsing (a bad key never breaks the rest)
  stats.rs     totals, burn rate, cache hit ratio, savings for a span
  budget.rs    token budgets + shared $ limits, forecasts, once-per-period alerts
  status.rs    the status-bar line
  chart.rs     text column charts
  report.rs    markdown report, breakdowns
  dashboard.rs find/start tokenburn-gui and tokenburn-web on the status bar's view
  engine.rs    one refresh: read → line + new alerts
  lsp.rs       the language server (progress, notifications, code actions)
  mcp.rs       the MCP server
fuzz/          settings_json, mcp_message, file_uri, chart_render
scripts/       nushell: bump, versions, package, release notes, quality gate (+ tests)
```

CI (GitHub and Gitea): quality gate, tests on macOS and Windows, static musl build,
MSRV, nightly `cargo audit` + `cargo deny`, nightly fuzzing, nightly lockfile updates,
Dependabot for actions. Commits follow [Conventional Commits](https://www.conventionalcommits.org)
(`feat`, `fix`, …, plus `lsp:` / `mcp:` / `ext:`); `CHANGELOG.md` is generated by
[git-cliff](https://git-cliff.org) — don't edit it by hand.

### Releasing

```sh
just release 0.2.0
```

bumps `Cargo.toml`, `extension.toml` **and** `mcp/extension.toml` (CI refuses a tag
where they differ),
regenerates the changelog, commits, tags and pushes. The tag builds the five binaries
the extension downloads, creates the GitHub release and publishes `tokenburn-zed` to
crates.io (`CRATES_IO_TOKEN`).

### Publishing the extension

Once the first release exists, add both to
[zed-industries/extensions](https://github.com/zed-industries/extensions) — **one PR
per extension** (the registry's rule), both pointing at the same submodule. See
[`docs/publishing.md`](docs/publishing.md) for the exact steps and PR texts.

## Related

| | |
|---|---|
| [tokenburn](https://github.com/sorinirimies/tokenburn) | CLI, TUI, desktop GUI and web dashboard — same core, same settings |
| [pi-tokenburn](https://github.com/sorinirimies/pi-tokenburn) | the live counter in pi's footer |

MIT
