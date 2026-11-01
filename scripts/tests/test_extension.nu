#!/usr/bin/env nu
# ── TokenBurn for Zed · test_extension.nu ────────────────────────────────────
# The extension manifest and the context-server configuration files are what
# Zed reads; keep them consistent with the code.

use std/assert
use runner.nu *

def root [] { $env.CURRENT_FILE | path dirname | path join ".." ".." }

def "test extension: manifest declares both servers" [] {
    let ext = (open (root | path join "extension.toml"))
    assert equal $ext.id "tokenburn"
    assert equal $ext.schema_version 1
    assert ("tokenburn" in ($ext.language_servers | columns))
    assert ("tokenburn" in ($ext.context_servers | columns))
}

def "test extension: the language server runs for common languages" [] {
    let langs = (open (root | path join "extension.toml") | get language_servers.tokenburn.languages)
    for l in ["Rust" "Python" "TypeScript" "Markdown" "JSON" "Plain Text" "Nu" "TOML"] {
        assert ($l in $langs) $"($l) is listed"
    }
    assert equal ($langs | uniq | length) ($langs | length) "no duplicates"
}

def "test extension: downloads only from this repository" [] {
    let caps = (open (root | path join "extension.toml") | get capabilities)
    assert equal ($caps | length) 1
    assert equal $caps.0.kind "download_file"
    assert equal $caps.0.host "github.com"
    assert equal $caps.0.path ["sorinirimies" "zed-tokenburn" "**"]
    let src = (open --raw (root | path join "src" "lib.rs"))
    assert ($src | str contains 'const REPO: &str = "sorinirimies/zed-tokenburn";')
}

def "test extension: settings schema covers every documented key" [] {
    let schema = (open (root | path join "configuration" "settings_schema.json"))
    let keys = ($schema.properties | columns)
    for k in ["enabled" "period" "tool" "refresh_secs" "include_cache" "show_cost" "show_rate" "show_hit" "show_saved" "currency" "budgets" "warn_pct" "notify" "desktop_notifications" "code_actions" "dashboard"] {
        assert ($k in $keys) $"schema has ($k)"
    }
    let config = (open --raw (root | path join "crates" "tokenburn-zed" "src" "config.rs"))
    for k in $keys {
        assert ($config | str contains $'"($k)"') $"config.rs reads ($k)"
    }
}

def "test extension: default settings are valid against the schema keys" [] {
    let raw = (open --raw (root | path join "configuration" "default_settings.jsonc"))
    let json = ($raw | lines | where {|l| not ($l | str trim | str starts-with "//") } | str join "\n" | from json)
    let keys = (open (root | path join "configuration" "settings_schema.json") | get properties | columns)
    for k in ($json | columns) {
        assert ($k in $keys) $"default key ($k) is in the schema"
    }
}

def main [] { run-tests }
