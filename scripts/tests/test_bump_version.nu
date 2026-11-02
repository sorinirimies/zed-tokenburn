#!/usr/bin/env nu
# ── TokenBurn for Zed · test_bump_version.nu ─────────────────────────────────
# Tests for the pure helpers of scripts/bump_version.nu.

use std/assert
use runner.nu *
use ../bump_version.nu [is_semver compare_versions set_workspace_version set_extension_version]

const CARGO = '[package]
name = "zed-tokenburn"
version.workspace = true

[workspace.package]
version      = "0.1.0"
edition      = "2021"

[workspace.dependencies]
tokenburn-core    = "=0.3.13"
serde = { version = "1", features = ["derive"] }
'

const EXT = 'id = "tokenburn"
name = "TokenBurn"
version = "0.1.0"
schema_version = 1

[language_servers.tokenburn]
name = "TokenBurn"
'

def "test bump: semver accepts releases and pre-releases" [] {
    assert (is_semver "0.1.0")
    assert (is_semver "12.0.3-rc.1")
    assert (not (is_semver "v0.1.0"))
    assert (not (is_semver "0.1"))
    assert (not (is_semver "0.1.0.4"))
    assert (not (is_semver ""))
}

def "test bump: compare orders by major minor patch" [] {
    assert equal (compare_versions "0.2.0" "0.1.9") 1
    assert equal (compare_versions "0.1.9" "0.2.0") (-1)
    assert equal (compare_versions "1.0.0" "1.0.0") 0
    assert equal (compare_versions "0.10.0" "0.9.0") 1 "numeric, not lexical"
}

def "test bump: a pre-release sorts below its release" [] {
    assert equal (compare_versions "1.0.0-rc.1" "1.0.0") (-1)
    assert equal (compare_versions "1.0.0" "1.0.0-rc.1") 1
    assert equal (compare_versions "1.0.0-rc.2" "1.0.0-rc.10") (-1)
    assert equal (compare_versions "1.0.0-alpha" "1.0.0-1") 1
    assert equal (compare_versions "1.0.0-rc" "1.0.0-rc.1") (-1)
}

def "test bump: only the workspace.package version changes" [] {
    let out = (set_workspace_version $CARGO "0.2.0")
    assert ($out | str contains 'version      = "0.2.0"')
    assert ($out | str contains 'tokenburn-core    = "=0.3.13"') "dependency pins stay"
    assert ($out | str contains 'serde = { version = "1"') "dependency versions stay"
    assert ($out | str contains 'version.workspace = true')
    assert equal ($out | from toml | get workspace.package.version) "0.2.0"
}

def "test bump: extension version changes once" [] {
    let out = (set_extension_version $EXT "0.2.0")
    assert equal ($out | from toml | get version) "0.2.0"
    assert equal ($out | from toml | get language_servers.tokenburn.name) "TokenBurn"
    assert equal ($out | lines | where {|l| $l starts-with "version" } | length) 1
}

def "test bump: setting the same version is a no-op on content" [] {
    let out = (set_extension_version $EXT "0.1.0")
    assert equal ($out | from toml) ($EXT | from toml)
}

def main [] { run-tests }
