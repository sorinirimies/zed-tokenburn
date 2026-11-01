//! TokenBurn MCP server for Zed — the WebAssembly side.
//!
//! Provides one context server, `tokenburn`: the native `tokenburn-zed mcp`
//! (see `crates/tokenburn-zed`) with read-only usage tools for Zed's agent. The
//! binary is `context_servers.tokenburn.settings.binary_path` when set, else
//! downloaded from this repository's latest GitHub release. The code that finds
//! and downloads it is shared with the status-bar extension in `../src`.

#[path = "../../src/download.rs"]
mod download;
#[path = "../../src/platform.rs"]
mod platform;
#[path = "../../src/resolve.rs"]
mod resolve;

use zed_extension_api::{
    self as zed, settings::ContextServerSettings, ContextServerConfiguration, ContextServerId,
    Project, Result,
};

/// Id of the context server (key of `context_servers` in Zed's settings).
const SERVER_ID: &str = "tokenburn";

#[derive(Default)]
struct TokenBurnMcp {
    /// Path of the downloaded binary (inside the extension's work directory).
    cached: Option<String>,
}

impl zed::Extension for TokenBurnMcp {
    fn new() -> Self {
        Self::default()
    }

    fn context_server_command(
        &mut self,
        _id: &ContextServerId,
        project: &Project,
    ) -> Result<zed::Command> {
        let settings = ContextServerSettings::for_project(SERVER_ID, project)
            .ok()
            .and_then(|s| s.settings);
        let env = resolve::mcp_env(settings.as_ref())?;
        let command = match resolve::mcp_binary(settings.as_ref()) {
            resolve::Binary::Host(path) => path,
            resolve::Binary::Download => download::binary(
                &mut self.cached,
                |_| {},
                "context_servers.tokenburn.settings.binary_path",
            )?,
        };
        Ok(zed::Command {
            command,
            args: resolve::args_or(None, "mcp"),
            env,
        })
    }

    fn context_server_configuration(
        &mut self,
        _id: &ContextServerId,
        _project: &Project,
    ) -> Result<Option<ContextServerConfiguration>> {
        Ok(Some(ContextServerConfiguration {
            installation_instructions: include_str!(
                "../configuration/installation_instructions.md"
            )
            .to_string(),
            settings_schema: include_str!("../configuration/settings_schema.json").to_string(),
            default_settings: include_str!("../configuration/default_settings.jsonc").to_string(),
        }))
    }
}

zed::register_extension!(TokenBurnMcp);
