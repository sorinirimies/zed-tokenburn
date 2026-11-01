# TokenBurn (MCP)

Read-only AI token and cost usage for Zed's agent, from the local logs of pi, Zed,
Claude Code, Codex, Copilot CLI, Gemini CLI, OpenCode, Amp, Cline, Roo Code, Kilo Code
and Qwen Code. Nothing leaves your machine.

The `tokenburn-zed` binary is downloaded from the
[latest release](https://github.com/sorinirimies/zed-tokenburn/releases) the first
time it is needed. To use your own build instead:

```sh
cargo install tokenburn-zed
```

and set `"command": { "path": "/path/to/tokenburn-zed", "arguments": ["mcp"] }`.

Tools: `tokenburn_usage`, `tokenburn_report`, `tokenburn_chart`,
`tokenburn_breakdown`, `tokenburn_budgets`. The settings below are optional and the
same as the status bar's (`lsp.tokenburn.settings`).
