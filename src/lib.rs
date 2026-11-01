//! TokenBurn for Zed — the WebAssembly side of the status bar.
//!
//! Zed extensions run as sandboxed WebAssembly and cannot draw UI, so all the real
//! work happens in the native `tokenburn-zed` binary (see `crates/tokenburn-zed`).
//! This crate only tells Zed how to start it as the **`tokenburn` language
//! server**: Zed shows a language server's progress message in the status bar,
//! which is where the live token counter appears. (The MCP tools for Zed's agent
//! are a separate extension, `mcp/`.)
//!
//! The binary is found, in order: `lsp.tokenburn.binary.path` in Zed's settings,
//! `tokenburn-zed` on the worktree's `PATH`, or downloaded from this repository's
//! latest GitHub release (once, then cached next to the extension).

mod download;
mod platform;
mod resolve;

use zed_extension_api::{
    self as zed, serde_json, settings::LspSettings, LanguageServerId, Result, Worktree,
};

/// Id of the language server (key of `lsp` in Zed's settings).
const SERVER_ID: &str = "tokenburn";

#[derive(Default)]
struct TokenBurnExtension {
    /// Path of the downloaded binary (inside the extension's work directory).
    cached: Option<String>,
}

impl zed::Extension for TokenBurnExtension {
    fn new() -> Self {
        Self::default()
    }

    fn language_server_command(
        &mut self,
        id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        let binary = LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.binary);
        let args = resolve::args_or(binary.as_ref().and_then(|b| b.arguments.clone()), "lsp");
        let env = binary
            .as_ref()
            .and_then(|b| b.env.clone())
            .map(|e| e.into_iter().collect())
            .unwrap_or_default();
        let command =
            match resolve::lsp_binary(binary.and_then(|b| b.path), worktree.which(resolve::BINARY))
            {
                resolve::Binary::Host(path) => path,
                resolve::Binary::Download => download::binary(
                    &mut self.cached,
                    |s| zed::set_language_server_installation_status(id, s),
                    "lsp.tokenburn.binary.path",
                )?,
            };
        Ok(zed::Command { command, args, env })
    }

    fn language_server_initialization_options(
        &mut self,
        _id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.initialization_options))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(LspSettings::for_worktree(SERVER_ID, worktree)
            .ok()
            .and_then(|s| s.settings))
    }
}

zed::register_extension!(TokenBurnExtension);
