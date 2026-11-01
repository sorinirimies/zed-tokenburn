#!/usr/bin/env nu
# ── TokenBurn for Zed · test_check_versions.nu ───────────────────────────────────────

use std/assert
use runner.nu *
use ../ci/check_versions.nu [mismatches EXTRA_MANIFESTS]

def "test check_versions: all agree" [] {
    assert equal (mismatches "0.1.0" {}) []
    assert equal (mismatches "0.1.0" {"extension.toml": "0.1.0"} "v0.1.0") []
}

def "test check_versions: a manifest that lags is named" [] {
    let m = (mismatches "0.2.0" {"extension.toml": "0.1.0"})
    assert equal ($m | length) 1
    assert ($m.0 | str contains "extension.toml has 0.1.0")
}

def "test check_versions: the tag must match" [] {
    let m = (mismatches "0.2.0" {} "v0.3.0")
    assert equal ($m | length) 1
    assert ($m.0 | str contains "tag v0.3.0")
    assert equal (mismatches "0.2.0" {"a.toml": "0.1.0"} "v9.9.9" | length) 2
}

def "test check_versions: the repository agrees with itself" [] {
    let root = ($env.CURRENT_FILE | path dirname | path join ".." "..")
    let cargo = (open ($root | path join "Cargo.toml") | get workspace.package.version)
    let manifests = ($EXTRA_MANIFESTS | reduce --fold {} {|f, acc| $acc | insert $f (open ($root | path join $f) | get version) })
    assert equal (mismatches $cargo $manifests) []
}

def main [] { run-tests }
