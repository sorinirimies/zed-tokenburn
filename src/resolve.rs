//! Decisions behind starting `tokenburn-zed`, kept free of the Zed API so they are
//! unit-tested natively. Shared by the status-bar extension (`src/lib.rs`) and the
//! MCP server extension (`mcp/src/lib.rs`).

// Each extension uses only its half of these helpers.
#![allow(dead_code)]

use serde_json::Value;

pub const BINARY: &str = "tokenburn-zed";
pub const REPO: &str = "sorinirimies/zed-tokenburn";
/// Environment variable the MCP server reads its settings from.
pub const SETTINGS_ENV: &str = "TOKENBURN_ZED_SETTINGS";
/// Extension-only key of the MCP server settings: use this binary, don't download.
pub const BINARY_PATH_KEY: &str = "binary_path";

/// Where the binary comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binary {
    /// A path on the host (settings or PATH).
    Host(String),
    /// The latest GitHub release, downloaded into the extension's directory.
    Download,
}

fn non_blank(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Language server: `lsp.tokenburn.binary.path`, then `tokenburn-zed` on the
/// worktree's PATH, then the release download.
pub fn lsp_binary(explicit: Option<String>, on_path: Option<String>) -> Binary {
    non_blank(explicit)
        .or_else(|| non_blank(on_path))
        .map_or(Binary::Download, Binary::Host)
}

/// MCP server: `context_servers.tokenburn.settings.binary_path`, else the
/// release download (a context server has no worktree whose PATH to search).
pub fn mcp_binary(settings: Option<&Value>) -> Binary {
    let path = settings
        .and_then(|s| s.get(BINARY_PATH_KEY))
        .and_then(Value::as_str)
        .map(str::to_string);
    non_blank(path).map_or(Binary::Download, Binary::Host)
}

/// Configured arguments, or the default mode.
pub fn args_or(args: Option<Vec<String>>, mode: &str) -> Vec<String> {
    args.unwrap_or_else(|| vec![mode.to_string()])
}

/// The MCP server's environment: its settings (minus the extension-only
/// `binary_path`) as JSON in [`SETTINGS_ENV`]; nothing when there are none.
pub fn mcp_env(settings: Option<&Value>) -> Result<Vec<(String, String)>, String> {
    let Some(Value::Object(map)) = settings else {
        return Ok(Vec::new());
    };
    let mut map = map.clone();
    map.remove(BINARY_PATH_KEY);
    if map.is_empty() {
        return Ok(Vec::new());
    }
    let json = serde_json::to_string(&Value::Object(map)).map_err(|e| e.to_string())?;
    Ok(vec![(SETTINGS_ENV.to_string(), json)])
}

/// Index of the release asset called `asset`, or an error telling the user how
/// to get the binary another way.
pub fn find_asset<'a>(
    mut names: impl Iterator<Item = &'a str>,
    asset: &str,
    version: &str,
    setting: &str,
) -> Result<usize, String> {
    names.position(|n| n == asset).ok_or_else(|| {
        format!(
            "release {version} of {REPO} has no {asset}; build the binary with \
             `cargo install --git https://github.com/{REPO} {BINARY}` and set {setting}"
        )
    })
}

/// Directory a release version is unpacked into.
pub fn version_dir(version: &str) -> String {
    format!("{BINARY}-{version}")
}

/// An older unpacked release that can be deleted (`keep` is the current one).
pub fn is_stale(name: &str, keep: &str) -> bool {
    name != keep && name.starts_with(&format!("{BINARY}-"))
}

/// `rel` made absolute against the extension's directory (Zed starts the binary
/// from elsewhere); unchanged when the directory is unknown.
pub fn absolute(cwd: Option<&std::path::Path>, rel: &str) -> String {
    cwd.map_or_else(
        || rel.to_string(),
        |d| d.join(rel).to_string_lossy().to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lsp_binary_prefers_settings_then_path_then_download() {
        assert_eq!(
            lsp_binary(Some("/s/tb".into()), Some("/p/tb".into())),
            Binary::Host("/s/tb".into())
        );
        assert_eq!(
            lsp_binary(None, Some("/p/tb".into())),
            Binary::Host("/p/tb".into())
        );
        assert_eq!(
            lsp_binary(Some("  ".into()), Some("/p/tb".into())),
            Binary::Host("/p/tb".into()),
            "a blank setting is no setting"
        );
        assert_eq!(lsp_binary(None, None), Binary::Download);
        assert_eq!(lsp_binary(Some(String::new()), None), Binary::Download);
    }

    #[test]
    fn mcp_binary_comes_from_binary_path_or_the_release() {
        let s = json!({ "binary_path": " /opt/tokenburn-zed ", "period": "week" });
        assert_eq!(
            mcp_binary(Some(&s)),
            Binary::Host("/opt/tokenburn-zed".into())
        );
        for s in [
            json!({}),
            json!({ "binary_path": "" }),
            json!({ "binary_path": 3 }),
            json!([]),
        ] {
            assert_eq!(mcp_binary(Some(&s)), Binary::Download, "{s}");
        }
        assert_eq!(mcp_binary(None), Binary::Download);
    }

    #[test]
    fn args_default_to_the_mode() {
        assert_eq!(args_or(None, "lsp"), ["lsp"]);
        assert_eq!(
            args_or(Some(vec!["mcp".into(), "-v".into()]), "lsp"),
            ["mcp", "-v"]
        );
        assert!(args_or(Some(vec![]), "lsp").is_empty(), "explicitly none");
    }

    #[test]
    fn mcp_env_passes_settings_without_binary_path() {
        let s = json!({ "binary_path": "/x", "period": "week", "budgets": { "day": 5 } });
        let env = mcp_env(Some(&s)).unwrap();
        assert_eq!(env.len(), 1);
        assert_eq!(env[0].0, SETTINGS_ENV);
        let back: Value = serde_json::from_str(&env[0].1).unwrap();
        assert_eq!(back, json!({ "period": "week", "budgets": { "day": 5 } }));

        assert!(mcp_env(None).unwrap().is_empty());
        assert!(mcp_env(Some(&json!({}))).unwrap().is_empty());
        assert!(mcp_env(Some(&json!({ "binary_path": "/x" })))
            .unwrap()
            .is_empty());
        assert!(mcp_env(Some(&json!("nope"))).unwrap().is_empty());
    }

    #[test]
    fn find_asset_or_explain() {
        let names = ["a.tar.gz", "tokenburn-zed-x86_64-apple-darwin.tar.gz"];
        assert_eq!(
            find_asset(names.into_iter(), names[1], "v1", "x").unwrap(),
            1
        );
        let e = find_asset(
            names.into_iter(),
            "missing.zip",
            "v0.1.0",
            "lsp.tokenburn.binary.path",
        )
        .unwrap_err();
        assert!(e.contains("v0.1.0") && e.contains("missing.zip"), "{e}");
        assert!(e.contains("cargo install --git"), "{e}");
        assert!(e.contains("lsp.tokenburn.binary.path"), "{e}");
    }

    #[test]
    fn old_releases_are_cleaned_up_but_nothing_else() {
        let keep = version_dir("v0.2.0");
        assert_eq!(keep, "tokenburn-zed-v0.2.0");
        assert!(is_stale("tokenburn-zed-v0.1.0", &keep));
        assert!(!is_stale(&keep, &keep));
        assert!(!is_stale("other-v0.1.0", &keep));
        assert!(!is_stale("tokenburn-zed", &keep));
    }

    #[test]
    fn absolute_joins_the_extension_dir() {
        let d = std::path::Path::new("/work/ext");
        assert_eq!(
            absolute(Some(d), "tb-v1/tokenburn-zed"),
            "/work/ext/tb-v1/tokenburn-zed"
        );
        assert_eq!(absolute(None, "tb-v1/tokenburn-zed"), "tb-v1/tokenburn-zed");
    }
}
