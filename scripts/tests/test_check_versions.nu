#!/usr/bin/env nu
# ── TokenBurn for Zed · test_check_versions.nu ───────────────────────────────

use std/assert
use runner.nu *
use ../ci/check_versions.nu [mismatches]

def "test versions: equal versions pass" [] {
    assert equal (mismatches "0.1.0" "0.1.0") []
    assert equal (mismatches "0.1.0" "0.1.0" "v0.1.0") []
}

def "test versions: extension.toml lagging behind is reported" [] {
    let m = (mismatches "0.2.0" "0.1.0")
    assert equal ($m | length) 1
    assert ($m.0 | str contains "extension.toml has 0.1.0")
}

def "test versions: a tag that does not match is reported" [] {
    let m = (mismatches "0.2.0" "0.2.0" "v0.3.0")
    assert equal ($m | length) 1
    assert ($m.0 | str contains "tag v0.3.0")
}

def "test versions: both problems at once" [] {
    assert equal (mismatches "0.2.0" "0.1.0" "v9.9.9" | length) 2
}

def "test versions: the repository itself agrees" [] {
    let root = ($env.CURRENT_FILE | path dirname | path join ".." "..")
    let cargo = (open ($root | path join "Cargo.toml") | get workspace.package.version)
    let ext = (open ($root | path join "extension.toml") | get version)
    assert equal (mismatches $cargo $ext) []
}

def main [] { run-tests }
