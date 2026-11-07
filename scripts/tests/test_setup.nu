#!/usr/bin/env nu
# ── TokenBurn for Zed · test_setup.nu ────────────────────────────────────────
# The decisions scripts/setup.nu makes, without network, cargo or Zed.

use std/assert
use runner.nu *
use ../setup.nu [release_target sources release_url checksum_of zed_extensions_dir cargo_bin_dir link_action extension_links mcp_settings]

const root = path self ../..

def "test setup: every supported platform has a release target" [] {
    assert equal (release_target "macos" "aarch64") "aarch64-apple-darwin"
    assert equal (release_target "macos" "x86_64") "x86_64-apple-darwin"
    assert equal (release_target "linux" "aarch64") "aarch64-unknown-linux-musl"
    assert equal (release_target "linux" "x86_64") "x86_64-unknown-linux-musl"
    assert equal (release_target "windows" "x86_64") "x86_64-pc-windows-msvc"
    assert equal (release_target "windows" "aarch64") "x86_64-pc-windows-msvc" "Windows on ARM emulates x86-64"
}

def "test setup: an unsupported platform says how to build it" [] {
    let e = try { release_target "linux" "x86" } catch {|err| $err.msg }
    assert str contains $e "cargo install tokenburn-zed"
}

def "test setup: the release targets match the extension" [] {
    let rust = open --raw ($root | path join src platform.rs)
    for t in ["aarch64-apple-darwin" "x86_64-apple-darwin" "aarch64-unknown-linux-musl" "x86_64-unknown-linux-musl" "x86_64-pc-windows-msvc"] {
        assert str contains $rust $"\"($t)\""
    }
}

def "test setup: crates io comes first and github is the fallback" [] {
    assert equal (sources "crates" true) ["crates" "github"]
    assert equal (sources "crates" false) ["github"] "without cargo only the release works"
    assert equal (sources "github" true) ["github"]
    assert error { sources "npm" true }
}

def "test setup: release urls point at the latest or a given version" [] {
    let t = "aarch64-apple-darwin"
    assert equal (release_url $t "") "https://github.com/sorinirimies/zed-tokenburn/releases/latest/download/tokenburn-zed-aarch64-apple-darwin.tar.gz"
    assert equal (release_url $t "0.1.1") "https://github.com/sorinirimies/zed-tokenburn/releases/download/v0.1.1/tokenburn-zed-aarch64-apple-darwin.tar.gz"
    assert equal (release_url $t "v0.1.1") (release_url $t "0.1.1") "a leading v is fine"
    assert (release_url "x86_64-pc-windows-msvc" "" | str ends-with ".zip")
}

def "test setup: the checksum is the first field of the sha256 file" [] {
    assert equal (checksum_of "ABC123  tokenburn-zed-x.tar.gz\n") "abc123"
    assert equal (checksum_of "abc123") "abc123"
}

def "test setup: zed keeps extensions in the per os data directory" [] {
    assert equal (zed_extensions_dir "macos" "/Users/me" {}) ("/Users/me" | path join "Library" "Application Support" "Zed" "extensions")
    assert equal (zed_extensions_dir "linux" "/home/me" {}) ("/home/me" | path join ".local" "share" "zed" "extensions")
    assert equal (zed_extensions_dir "linux" "/home/me" {XDG_DATA_HOME: "/data"}) ("/data" | path join "zed" "extensions")
    assert equal (zed_extensions_dir "linux" "/home/me" {XDG_DATA_HOME: ""}) ("/home/me" | path join ".local" "share" "zed" "extensions") "an empty XDG_DATA_HOME is unset"
    assert equal (zed_extensions_dir "windows" "C:/Users/me" {LOCALAPPDATA: "C:/Users/me/AppData/Local"}) ("C:/Users/me/AppData/Local" | path join "Zed" "extensions")
}

def "test setup: binaries go to the cargo bin directory" [] {
    assert equal (cargo_bin_dir "/home/me" {}) ("/home/me" | path join ".cargo" "bin")
    assert equal (cargo_bin_dir "/home/me" {CARGO_HOME: "/opt/cargo"}) ("/opt/cargo" | path join "bin")
    assert equal (cargo_bin_dir "/home/me" {CARGO_HOME: ""}) ("/home/me" | path join ".cargo" "bin")
}

def "test setup: links are created kept or replaced and installs are left alone" [] {
    assert equal (link_action null "" "/repo") "create"
    assert equal (link_action "symlink" "/repo" "/repo") "keep"
    assert equal (link_action "symlink" "/old/clone" "/repo") "replace"
    assert equal (link_action "symlink" "" "/repo") "replace" "a broken link is replaced"
    assert equal (link_action "dir" "" "/repo") "blocked" "a registry install is never touched"
    assert equal (link_action "file" "" "/repo") "blocked"
}

def "test setup: both extensions are linked under their ids" [] {
    let links = extension_links $root "/zed"
    assert equal ($links | get id) ["tokenburn" "tokenburn-mcp-server"]
    assert equal ($links | get link) [("/zed" | path join "installed" "tokenburn") ("/zed" | path join "installed" "tokenburn-mcp-server")]
    assert equal ($links.1.dir) ($root | path join "mcp" | path expand)
}

def "test setup: the mcp setting points at the binary" [] {
    let s = mcp_settings "/home/me/.cargo/bin/tokenburn-zed" | from json
    assert equal $s.context_servers.tokenburn.settings.binary_path "/home/me/.cargo/bin/tokenburn-zed"
}

def main [] { run-tests }
