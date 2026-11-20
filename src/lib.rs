//! TokenBurn for Zed — the WebAssembly side of the status bar.
//!
//! Zed extensions run as sandboxed WebAssembly and cannot draw UI, so all the real
//! work happens in the native `tokenburn-zed` binary (see `crates/tokenburn-zed`).
//! This crate only tells Zed how to start it as the **`TokenBurn` language
//! server**: Zed shows a language server's progress message in the status bar,
//! which is where the live token counter appears. (The MCP tools for Zed's agent
//! are a separate extension, `mcp/`.)
//!
//! The binary is found, in order: `lsp.TokenBurn.binary.path` in Zed's settings
//! (or the older `lsp.tokenburn.binary.path`),
//! `tokenburn-zed` on the worktree's `PATH`, or downloaded from this repository's
//! latest GitHub release (once, then cached next to the extension).

mod download;
mod platform;
mod resolve;

use zed_extension_api::{
    self as zed, serde_json, settings::LspSettings, LanguageServerId, Result, Worktree,
};

/// Id of the language server (key of `lsp` in Zed's settings). Zed prints it before
/// the idle status text, `(TokenBurn) 🔥 …`, so it is spelled like the progress title.
const SERVER_ID: &str = "TokenBurn";

/// The id before 0.1.5. Its settings are still read, under the new ones.
const LEGACY_SERVER_ID: &str = "tokenburn";

fn lsp_settings(id: &str, worktree: &Worktree) -> Option<LspSettings> {
    LspSettings::for_worktree(id, worktree).ok()
}

/// `lsp.<id>.binary` of the current id, then of the legacy one, field by field.
fn binary_settings(worktree: &Worktree) -> resolve::Command {
    let read = |id: &str| {
        lsp_settings(id, worktree)
            .and_then(|s| s.binary)
            .map(|b| resolve::Command {
                path: b.path,
                arguments: b.arguments,
                env: b.env.map(|e| e.into_iter().collect()),
            })
            .unwrap_or_default()
    };
    resolve::merge_command(read(SERVER_ID), read(LEGACY_SERVER_ID))
}

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
        let binary = binary_settings(worktree);
        let args = resolve::args_or(binary.arguments.clone(), "lsp");
        let env = binary.env.clone().unwrap_or_default();
        let command = match resolve::lsp_binary(binary.path, worktree.which(resolve::BINARY)) {
            resolve::Binary::Host(path) => path,
            resolve::Binary::Download => download::binary(
                &mut self.cached,
                |s| zed::set_language_server_installation_status(id, s),
                "lsp.TokenBurn.binary.path",
            )?,
        };
        Ok(zed::Command { command, args, env })
    }

    fn language_server_initialization_options(
        &mut self,
        _id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(resolve::merge_json(
            lsp_settings(SERVER_ID, worktree).and_then(|s| s.initialization_options),
            lsp_settings(LEGACY_SERVER_ID, worktree).and_then(|s| s.initialization_options),
        ))
    }

    fn language_server_workspace_configuration(
        &mut self,
        _id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<serde_json::Value>> {
        Ok(resolve::merge_json(
            lsp_settings(SERVER_ID, worktree).and_then(|s| s.settings),
            lsp_settings(LEGACY_SERVER_ID, worktree).and_then(|s| s.settings),
        ))
    }
}

zed::register_extension!(TokenBurnExtension);
