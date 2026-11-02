---
name: zed-tokenburn-rules
description: Rules for work in the zed-tokenburn repository that the code can't show: constraints, ownership and must-keep behaviour. Use before changing, fixing, reviewing or extending code here, together with the rust-in-projects skill.
---

# zed-tokenburn: rules the code can't show

Hard constraints for this repository. The general conventions (orientation, tests,
scripting, releasing) are in the `rust-in-projects` skill; this file holds only what is
specific to zed-tokenburn.

* **Two extensions in one repo.**
  * Root `extension.toml` (id `tokenburn`, `src/`) holds only the language server
    that draws the status bar.
  * `mcp/` (id `tokenburn-mcp-server`) holds exactly one context server and
    nothing else, because the registry requires it (a nu test enforces this).
  * Both only find or download the native binary, `crates/tokenburn-zed`.
    `mcp/` shares code through `#[path = "../../src/…"]`.
* **No log parsing here.** `tokenburn-core` is pinned `=0.3.7`, because newer
  versions pull in the vulnerable zstd. A new log source goes into tokenburn-core
  first.
* **The language server never touches documents.**
  * No diagnostics, no edits.
  * Code actions are plain `Command`s, offered on line 1 only.
  * They are offered only when the request has no `context.only`, so
    format-on-save never runs them.
* **Dashboards are tokenburn's own.**
  * Start `tokenburn-gui` / `tokenburn-web`, without flags that persist shared
    settings (`--currency`).
  * Reuse a running web server only when `/api/themes` proves it is tokenburn-web.
* **Settings are lenient.** A bad value falls back to the default for that key
  only.
* **New setting key** ⇒ `config.rs` + `mcp/configuration/settings_schema.json` +
  README (a nu test checks the schema against the code).
* **Shared state:** dollar limits, currency and the cost alert log come from
  tokenburn-core's `settings.json` / `alerts.json`. Zed-only token alerts go in
  `zed-alerts.json`.
* **LSP tests** go through `Connection::memory()` with an injected `Refresher`.
  **stdio tests** use a fake `HOME`, `TOKENBURN_CONFIG_DIR` and the like, never
  real logs.
* **Release asset names** are defined in `src/platform.rs` and
  `scripts/ci/package.nu`; change both (a nu test compares them).
* **Commit prefixes:** `lsp:` / `mcp:` / `ext:` on top of the conventional
  types.
