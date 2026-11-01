//! Download the `tokenburn-zed` release binary for this machine (Zed API side;
//! the decisions live in `resolve.rs` and `platform.rs`).

use std::fs;

use zed_extension_api::{self as zed, LanguageServerInstallationStatus as Status, Result};

use crate::{platform, resolve};

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

/// The latest release binary (absolute path), downloading it when it is not in
/// `cache` / on disk yet. `status` reports progress (language servers only);
/// `setting` names the setting that avoids the download, for error messages.
pub fn binary(
    cache: &mut Option<String>,
    status: impl Fn(&Status),
    setting: &str,
) -> Result<String> {
    if let Some(path) = cache.as_ref() {
        if fs::metadata(path).is_ok_and(|m| m.is_file()) {
            return Ok(path.clone());
        }
    }
    status(&Status::CheckingForUpdate);

    let (os, arch) = zed::current_platform();
    let target = platform::target(platform_os(os), platform_arch(arch))?;
    let release = zed::latest_github_release(
        resolve::REPO,
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        },
    )?;
    let asset_name = platform::asset_name(target);
    let index = resolve::find_asset(
        release.assets.iter().map(|a| a.name.as_str()),
        &asset_name,
        &release.version,
        setting,
    )?;
    let asset = &release.assets[index];

    let dir = resolve::version_dir(&release.version);
    let rel = platform::binary_path(&dir, target);
    if !fs::metadata(&rel).is_ok_and(|m| m.is_file()) {
        status(&Status::Downloading);
        let kind = if platform::is_windows(target) {
            zed::DownloadedFileType::Zip
        } else {
            zed::DownloadedFileType::GzipTar
        };
        zed::download_file(&asset.download_url, &dir, kind)
            .map_err(|e| format!("downloading {asset_name}: {e}"))?;
        zed::make_file_executable(&rel)?;
        if let Ok(entries) = fs::read_dir(".") {
            for e in entries.flatten() {
                if resolve::is_stale(&e.file_name().to_string_lossy(), &dir) {
                    let _ = fs::remove_dir_all(e.path());
                }
            }
        }
    }
    status(&Status::None);

    let abs = resolve::absolute(std::env::current_dir().ok().as_deref(), &rel);
    *cache = Some(abs.clone());
    Ok(abs)
}
