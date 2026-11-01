# Publishing to the Zed extension registry

Two extensions live in this repository and are published separately (the registry
accepts **exactly one extension per PR**):

| id | directory | provides |
|---|---|---|
| `tokenburn` | `.` | language server `tokenburn` (the status bar) |
| `tokenburn-mcp-server` | `mcp/` | context server `tokenburn` (agent tools) |

## 1. Release first

Both extensions download `tokenburn-zed` from this repository's latest GitHub release,
so a release must exist before anyone can install them.

```sh
just check-all          # quality gate, incl. the Zed-style build of both extensions
just push-all           # GitHub + Gitea
just release-initial    # tag v0.1.0 → release workflow builds the five binaries
```

Check the release has `tokenburn-zed-{aarch64,x86_64}-apple-darwin.tar.gz`,
`tokenburn-zed-{aarch64,x86_64}-unknown-linux-musl.tar.gz` and
`tokenburn-zed-x86_64-pc-windows-msvc.zip`.

## 2. Test at the commit you submit

The registry requires a manual test at the submitted commit. Remove any
`lsp.tokenburn.binary.path` / `binary_path` from your Zed settings so the download
path is exercised, then:

1. `zed: install dev extension` → this folder. Open a file in a trusted project; the
   status bar shows `TokenBurn: 🔥 …`; `cmd-.` on line 1 lists the TokenBurn actions.
2. `zed: install dev extension` → `mcp/`. Agent settings → MCP servers → enable
   *tokenburn*; ask the agent *"use tokenburn_usage for this week"*.
3. Zed log (`zed: open log`) has no `tokenburn` errors.

## 3. Open the PRs

```sh
gh repo fork zed-industries/extensions --clone --remote   # fork to your personal account
cd extensions
git submodule update --init --recursive --depth 1
git submodule add https://github.com/sorinirimies/zed-tokenburn.git extensions/tokenburn
```

**PR 1** — branch `tokenburn`, add to `extensions.toml`:

```toml
[tokenburn]
submodule = "extensions/tokenburn"
version = "0.1.0"
```

**PR 2** — branch `tokenburn-mcp-server` (after PR 1 is merged, or reusing the same
submodule path), add:

```toml
[tokenburn-mcp-server]
submodule = "extensions/tokenburn"
path = "mcp"
version = "0.1.0"
```

For each: `pnpm sort-extensions`, commit, push, `gh pr create`.

### PR text — `tokenburn`

> Adds **TokenBurn**: live AI token and cost usage (pi, Zed's agent, Claude Code, Codex,
> Copilot CLI, Gemini CLI, OpenCode, Amp, Cline, Roo, Kilo, Qwen) in the status bar,
> from local logs only — nothing leaves the machine.
>
> A note on the approach, since it touches the "don't work around API limitations"
> guideline: the extension API has no status-bar item or command API, so the extension
> starts a language server that never reads or edits documents (no diagnostics, no
> edits, code actions only as plain commands and never for `context.only` requests) and
> keeps one work-done progress open whose message is the usage line. It is registered
> for many languages only so it runs whatever file is open. Happy to switch to a
> status-bar API as soon as one exists.
>
> The native server (`tokenburn-zed`, MIT, in the same repository) is downloaded from
> the GitHub releases, or taken from `PATH` / `lsp.tokenburn.binary.path`; it is not
> bundled. Tested manually at the submitted commit on macOS.

### PR text — `tokenburn-mcp-server`

> Adds **TokenBurn MCP Server**: one context server with read-only tools for the agent
> (`tokenburn_usage`, `tokenburn_report`, `tokenburn_chart`, `tokenburn_breakdown`,
> `tokenburn_budgets`) over local AI tool logs. The server binary is downloaded from the
> repository's GitHub releases (or `settings.binary_path`), not bundled. It lives in the
> `mcp/` subdirectory with its own LICENSE (MIT). Tested manually at the submitted
> commit on macOS.

## Updates

`just release X.Y.Z` bumps all three versions. Then in the extensions fork: update the
submodule to the tagged commit and both `version`s in `extensions.toml` — again one PR
per extension.

If a maintainer asks for a different id for the language-server extension (the
guideline suggests `-lsp` for language-server-only extensions), rename `id` in
`extension.toml` and the registry entry; user settings are unaffected because they key
on the language server id `tokenburn`, not the extension id.
