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
| **Report** | a code action opens a markdown report in a tab: usage for every period, budgets, daily / weekly / monthly charts, usage by tool and by project, savings by source |
| **Savings** | prompt cache, [rtk](https://github.com/rtk-ai/rtk) and pi compaction, valued at your input price |
| **Agent tools (MCP)** | Zed's agent can answer "what did I spend this week?" itself: `tokenburn_usage`, `tokenburn_report`, `tokenburn_chart`, `tokenburn_breakdown`, `tokenburn_budgets` |
| **Filters** | one tool or all, with or without cache tokens, any currency TokenBurn knows |

Costs are what the tools log; **Zed, Codex, Gemini, Copilot, Amp and Qwen log none**, so
their cost is `$0.00` rather than a guess (token counts are exact).

## How it works

Zed extensions run as sandboxed WebAssembly and have **no API for status-bar items or
commands**. What Zed *does* show in its status bar is a language server's work-done
progress. So the extension is two pieces:

```
┌────────────── Zed ──────────────┐        ┌──────── tokenburn-zed (native) ───────┐
│ extension (wasm, src/)          │ starts │ lsp: one $/progress kept open,        │
│  • language server "tokenburn"  ├───────▶│      message = the status line        │──▶ tokenburn-core
│  • context server  "tokenburn"  │        │ mcp: read-only usage tools            │    (local logs)
└─────────────────────────────────┘        └───────────────────────────────────────┘
```

* The **language server** never reads or changes your code. It is registered for ~260
  languages only so it runs whatever file is open. It keeps one progress titled
  `TokenBurn` and updates its message; alerts are `window/showMessage`; the code
  actions are `workspace/executeCommand`.
* The **context server** is a small MCP server on stdio.
* Both are the same binary, `tokenburn-zed` (`lsp` / `mcp` / `status` / `report`).

> The status bar shows the activity spinner next to the text (it is a "progress" to
> Zed), and when another language server is busy Zed shows the most recent work
> first. That is the price of having no status-bar API.

## Install

1. **Zed → Extensions** (`zed: extensions`) → search **TokenBurn** → Install.
2. Open any file. The extension downloads `tokenburn-zed` for your platform from the
   [latest release](https://github.com/sorinirimies/zed-tokenburn/releases) (macOS
   arm64/x86-64, Linux arm64/x86-64 static, Windows x86-64) and the status bar shows
   `TokenBurn: 🔥 …`.

Prefer your own build? Put it on `PATH` and the extension uses it instead:

```sh
cargo install tokenburn-zed
```

From source (dev extension):

```sh
just dev    # installs tokenburn-zed, builds the wasm
# Zed: command palette → "zed: install dev extension" → pick this folder
```

## Use

**Code actions** (`cmd-.` / `ctrl-.`, in any file):

| Action | |
|---|---|
| 🔥 TokenBurn: open usage report | writes `~/Library/Caches/tokenburn/zed-report.md` (`~/.cache/…` on Linux) and opens it |
| 🔥 TokenBurn: status bar → week (now today) | cycles today → week → month → year → all → hour |
| 🔥 TokenBurn: pause / resume the status bar | paused = nothing read in the background |

Cycling and pausing last until Zed restarts the server or the settings change; put
permanent choices in the settings. Turn the actions off with `"code_actions": false`.

**Agent panel**: enable the *TokenBurn* context server (Agent settings → MCP servers)
and ask, e.g. *"Use tokenburn to show my daily token chart"*.

**Terminal**: `tokenburn-zed status --period week`, `tokenburn-zed report --tool claude`.

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
        "code_actions": true,
        "enabled": true
      }
      // "binary": { "path": "/path/to/tokenburn-zed", "arguments": ["lsp"] }
    }
  },
  "context_servers": {
    "tokenburn": {
      "settings": { "tool": "all" }  // same keys; used by the MCP tools
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
extension.toml, src/           the Zed extension (wasm32-wasip2): finds/downloads the binary
configuration/                 MCP server instructions, settings schema, defaults
crates/tokenburn-zed/src/
  span.rs      hour/today/week/month/year/all boundaries (local time, Monday weeks)
  config.rs    lenient settings parsing (a bad key never breaks the rest)
  stats.rs     totals, burn rate, cache hit ratio, savings for a span
  budget.rs    token budgets + shared $ limits, forecasts, once-per-period alerts
  status.rs    the status-bar line
  chart.rs     text column charts
  report.rs    markdown report, breakdowns
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

bumps `Cargo.toml` **and** `extension.toml` (CI refuses a tag where they differ),
regenerates the changelog, commits, tags and pushes. The tag builds the five binaries
the extension downloads, creates the GitHub release and publishes `tokenburn-zed` to
crates.io (`CRATES_IO_TOKEN`).

### Publishing the extension

Once the first release exists, add it to [zed-industries/extensions](https://github.com/zed-industries/extensions):

```sh
git submodule add https://github.com/sorinirimies/zed-tokenburn.git extensions/tokenburn
```

```toml
# extensions.toml
[tokenburn]
submodule = "extensions/tokenburn"
version = "0.1.0"
```

then `pnpm sort-extensions` and open a PR. For updates, bump the submodule and the
version.

## Related

| | |
|---|---|
| [tokenburn](https://github.com/sorinirimies/tokenburn) | CLI, TUI, desktop GUI and web dashboard — same core, same settings |
| [pi-tokenburn](https://github.com/sorinirimies/pi-tokenburn) | the live counter in pi's footer |

MIT
