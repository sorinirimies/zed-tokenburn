#!/usr/bin/env nu
# ── TokenBurn for Zed · test_check_versions.nu ───────────────────────────────

use std/assert
use runner.nu *
use ../ci/check_versions.nu [mismatches MANIFESTS]

def "test versions: equal versions pass" [] {
    let e = {"extension.toml": "0.1.0", "mcp/extension.toml": "0.1.0"}
    assert equal (mismatches "0.1.0" $e) []
    assert equal (mismatches "0.1.0" $e "v0.1.0") []
}

def "test versions: extension.toml lagging behind is reported" [] {
    let m = (mismatches "0.2.0" {"extension.toml": "0.1.0", "mcp/extension.toml": "0.2.0"})
    assert equal ($m | length) 1
    assert ($m.0 | str contains "extension.toml has 0.1.0")
    let m = (mismatches "0.2.0" {"extension.toml": "0.2.0", "mcp/extension.toml": "0.1.0"})
    assert equal ($m | length) 1
    assert ($m.0 | str contains "mcp/extension.toml has 0.1.0")
}

def "test versions: a tag that does not match is reported" [] {
    let m = (mismatches "0.2.0" {"extension.toml": "0.2.0"} "v0.3.0")
    assert equal ($m | length) 1
    assert ($m.0 | str contains "tag v0.3.0")
}

def "test versions: both problems at once" [] {
    let e = {"extension.toml": "0.1.0", "mcp/extension.toml": "0.1.0"}
    assert equal (mismatches "0.2.0" $e "v9.9.9" | length) 3
}

def "test versions: the repository itself agrees" [] {
    let root = ($env.CURRENT_FILE | path dirname | path join ".." "..")
    let cargo = (open ($root | path join "Cargo.toml") | get workspace.package.version)
    let exts = ($MANIFESTS | each {|f| {file: $f, version: (open ($root | path join $f) | get version)} }
        | transpose --header-row --as-record)
    assert equal ($exts | columns | length) 2
    assert equal (mismatches $cargo $exts) []
}

def main [] { run-tests }
