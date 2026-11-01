#!/usr/bin/env nu
# ── TokenBurn for Zed · test_extension.nu ────────────────────────────────────
# The two extension manifests and the context-server configuration files are what
# Zed reads; keep them consistent with the code and with Zed's publishing rules
# (an MCP server extension provides exactly one context server and nothing else).

use std/assert
use runner.nu *

def root [] { $env.CURRENT_FILE | path dirname | path join ".." ".." }

def "test extension: the status bar extension only provides the language server" [] {
    let ext = (open (root | path join "extension.toml"))
    assert equal $ext.id "tokenburn"
    assert equal $ext.schema_version 1
    assert equal ($ext.language_servers | columns) ["tokenburn"]
    assert ("context_servers" not-in ($ext | columns))
}

def "test extension: the MCP extension provides exactly one context server" [] {
    let ext = (open (root | path join "mcp" "extension.toml"))
    assert equal $ext.id "tokenburn-mcp-server"
    assert ($ext.id | str ends-with "-mcp-server")
    assert equal $ext.schema_version 1
    assert equal ($ext.context_servers | columns) ["tokenburn"]
    let other = ($ext | columns | where {|c| $c in ["language_servers" "grammars" "languages" "themes" "snippets" "debug_adapters" "slash_commands"] })
    assert equal $other [] "nothing else"
}

def "test extension: ids follow the registry rules" [] {
    for f in ["extension.toml" "mcp/extension.toml"] {
        let id = (open (root | path join $f) | get id)
        assert ($id =~ '^[a-z0-9]+(-[a-z0-9]+)*$') $"($id) is kebab-case"
        assert (not ($id | str contains "zed")) $"($id) has no 'zed'"
        assert (not ($id | str contains "extension")) $"($id) has no 'extension'"
    }
}

def "test extension: both extensions carry an accepted license" [] {
    for dir in ["." "mcp"] {
        let lic = (root | path join $dir "LICENSE")
        assert ($lic | path exists) $"($dir)/LICENSE exists"
        assert ((open --raw $lic | lines | first) == "MIT License") $"($dir) is MIT"
    }
}

def "test extension: Cargo workspace builds both extensions" [] {
    let cargo = (open (root | path join "Cargo.toml"))
    assert ("mcp" in $cargo.workspace.members)
    assert equal (open (root | path join "mcp" "Cargo.toml") | get lib.crate-type) ["cdylib"]
}

def "test extension: the language server runs for common languages" [] {
    let langs = (open (root | path join "extension.toml") | get language_servers.tokenburn.languages)
    for l in ["Rust" "Python" "TypeScript" "Markdown" "JSON" "Plain Text" "Nu" "TOML"] {
        assert ($l in $langs) $"($l) is listed"
    }
    assert equal ($langs | uniq | length) ($langs | length) "no duplicates"
}

def "test extension: downloads only from this repository" [] {
    for f in ["extension.toml" "mcp/extension.toml"] {
        let caps = (open (root | path join $f) | get capabilities)
        assert equal ($caps | length) 1
        assert equal $caps.0.kind "download_file"
        assert equal $caps.0.host "github.com"
        assert equal $caps.0.path ["sorinirimies" "zed-tokenburn" "**"]
    }
    let src = (open --raw (root | path join "src" "resolve.rs"))
    assert ($src | str contains 'pub const REPO: &str = "sorinirimies/zed-tokenburn";')
}

def "test extension: settings schema covers every documented key" [] {
    let schema = (open (root | path join "mcp" "configuration" "settings_schema.json"))
    let keys = ($schema.properties | columns | where $it != "binary_path")
    for k in ["enabled" "period" "tool" "refresh_secs" "include_cache" "show_cost" "show_rate" "show_hit" "show_saved" "currency" "budgets" "warn_pct" "notify" "desktop_notifications" "code_actions" "dashboard"] {
        assert ($k in $keys) $"schema has ($k)"
    }
    let config = (open --raw (root | path join "crates" "tokenburn-zed" "src" "config.rs"))
    for k in $keys {
        assert ($config | str contains $'"($k)"') $"config.rs reads ($k)"
    }
}

def "test extension: default settings are valid against the schema keys" [] {
    let raw = (open --raw (root | path join "mcp" "configuration" "default_settings.jsonc"))
    let json = ($raw | lines | where {|l| not ($l | str trim | str starts-with "//") } | str join "\n" | from json)
    let keys = (open (root | path join "mcp" "configuration" "settings_schema.json") | get properties | columns)
    for k in ($json | columns) {
        assert ($k in $keys) $"default key ($k) is in the schema"
    }
}

def "test extension: binary_path is a key of the MCP extension only" [] {
    let schema = (open (root | path join "mcp" "configuration" "settings_schema.json"))
    assert ("binary_path" in ($schema.properties | columns))
    let resolve = (open --raw (root | path join "src" "resolve.rs"))
    assert ($resolve | str contains 'BINARY_PATH_KEY: &str = "binary_path"')
}

def "test extension: the Zed-style build covers both crates" [] {
    use ../ci/zed_build.nu [EXTENSIONS wasm_path]
    assert equal ($EXTENSIONS | get dir) ["." "mcp"]
    for e in $EXTENSIONS {
        let manifest = (open (root | path join $e.dir "Cargo.toml"))
        assert equal ($manifest.package.name | str replace --all "-" "_") $e.crate
    }
    assert equal (wasm_path "mcp" "x") "mcp/target/wasm32-wasip2/release/x.wasm"
}

def main [] { run-tests }
