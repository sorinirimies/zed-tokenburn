//! TokenBurn for Zed — the WebAssembly side.
//!
//! Zed extensions run as sandboxed WebAssembly and cannot draw UI, so all the real
//! work happens in the native `tokenburn-zed` binary (see `crates/tokenburn-zed`).
//! This crate only tells Zed how to start it:
//!
//! * as the **`tokenburn` language server** — Zed shows its progress message in
//!   the status bar, which is where the live token counter appears;
//! * as the **`tokenburn` context server** (MCP) — usage tools for Zed's agent.
//!
//! The binary is found, in order: `lsp.tokenburn.binary.path` /
//! `context_servers.tokenburn.command.path` in Zed's settings, `tokenburn-zed`
//! on the worktree's `PATH`, or downloaded from this repository's latest GitHub
//! release (once, then cached next to the extension).

mod platform;

use std::fs;

use zed_extension_api::{
    self as zed, serde_json,
    settings::{ContextServerSettings, LspSettings},
    ContextServerConfiguration, ContextServerId, LanguageServerId, Project, Result, Worktree,
};

/// Id of the language server and of the context server (keys of `lsp` and
/// `context_servers` in Zed's settings).
const SERVER_ID: &str = "tokenburn";
const BINARY: &str = "tokenburn-zed";
const REPO: &str = "sorinirimies/zed-tokenburn";

#[derive(Default)]
struct TokenBurnExtension {
    /// Path of the downloaded (or found) binary, reused for both servers.
    cached: Option<String>,
}

impl TokenBurnExtension {
    /// The latest release binary, downloading it when it is not there yet.
    fn downloaded_binary(&mut self, lsp_id: Option<&LanguageServerId>) -> Result<String> {
        if let Some(path) = &self.cached {
            if fs::metadata(path).is_ok_and(|m| m.is_file()) {
                return Ok(path.clone());
            }
        }
        let status = |s: &zed::LanguageServerInstallationStatus| {
            if let Some(id) = lsp_id {
                zed::set_language_server_installation_status(id, s);
            }
        };
        status(&zed::LanguageServerInstallationStatus::CheckingForUpdate);

        let (os, arch) = zed::current_platform();
        let target = platform::target(platform_os(os), platform_arch(arch))?;
        let release = zed::latest_github_release(
            REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;
        let asset_name = platform::asset_name(target);
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| {
                format!(
                    "release {} of {REPO} has no {asset_name}; install the binary with \
                     `cargo install tokenburn-zed` or set lsp.tokenburn.binary.path",
                    release.version
                )
            })?;

        let dir = format!("{BINARY}-{}", release.version);
        let rel = platform::binary_path(&dir, target);
        if !fs::metadata(&rel).is_ok_and(|m| m.is_file()) {
            status(&zed::LanguageServerInstallationStatus::Downloading);
            let kind = if platform::is_windows(target) {
                zed::DownloadedFileType::Zip
            } else {
                zed::DownloadedFileType::GzipTar
            };
            zed::download_file(&asset.download_url, &dir, kind)
                .map_err(|e| format!("downloading {asset_name}: {e}"))?;
            zed::make_file_executable(&rel)?;
            // Drop older versions.
            if let Ok(entries) = fs::read_dir(".") {
                for e in entries.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with(&format!("{BINARY}-")) && name != dir {
                        let _ = fs::remove_dir_all(e.path());
                    }
                }
            }
        }
        status(&zed::LanguageServerInstallationStatus::None);

        let abs = std::env::current_dir()
            .map(|d| d.join(&rel).to_string_lossy().to_string())
            .unwrap_or(rel);
        self.cached = Some(abs.clone());
        Ok(abs)
    }
}

fn platform_os(os: zed::Os) -> platform::Os {
    match os {
        zed::Os::Mac => platform::Os::Mac,
        zed::Os::Linux => platform::Os::Linux,
        zed::Os::Windows => platform::Os::Windows,
    }
}

fn platform_arch(arch: zed::Architecture) -> platform::Arch {
    match arch {
        zed::Architecture::Aarch64 => platform::Arch::Aarch64,
        zed::Architecture::X86 => platform::Arch::X86,
        zed::Architecture::X8664 => platform::Arch::X86_64,
    }
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
        let args = binary
            .as_ref()
            .and_then(|b| b.arguments.clone())
            .unwrap_or_else(|| vec!["lsp".into()]);
        let env = binary
            .as_ref()
            .and_then(|b| b.env.clone())
            .map(|e| e.into_iter().collect())
            .unwrap_or_default();
        let command = match binary.and_then(|b| b.path) {
            Some(path) => path,
            None => match worktree.which(BINARY) {
                Some(path) => path,
                None => self.downloaded_binary(Some(id))?,
            },
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

    fn context_server_command(
        &mut self,
        _id: &ContextServerId,
        project: &Project,
    ) -> Result<zed::Command> {
        let settings = ContextServerSettings::for_project(SERVER_ID, project).unwrap_or_default();
        let mut env: Vec<(String, String)> = Vec::new();
        if let Some(s) = &settings.settings {
            env.push((
                "TOKENBURN_ZED_SETTINGS".into(),
                serde_json::to_string(s).map_err(|e| e.to_string())?,
            ));
        }
        let command = settings.command.unwrap_or(zed::settings::CommandSettings {
            path: None,
            arguments: None,
            env: None,
        });
        if let Some(extra) = command.env {
            env.extend(extra);
        }
        let path = match command.path {
            Some(p) => p,
            None => self.downloaded_binary(None)?,
        };
        Ok(zed::Command {
            command: path,
            args: command.arguments.unwrap_or_else(|| vec!["mcp".into()]),
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

zed::register_extension!(TokenBurnExtension);
