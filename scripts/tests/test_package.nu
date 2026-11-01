#!/usr/bin/env nu
# ── TokenBurn for Zed · test_package.nu ──────────────────────────────────────
# The archive names must match what the extension downloads (src/platform.rs).

use std/assert
use runner.nu *
use ../ci/package.nu [asset_name binary_name is_windows]

def "test package: unix targets are tar.gz" [] {
    assert equal (asset_name "aarch64-apple-darwin") "tokenburn-zed-aarch64-apple-darwin.tar.gz"
    assert equal (asset_name "x86_64-unknown-linux-musl") "tokenburn-zed-x86_64-unknown-linux-musl.tar.gz"
    assert equal (binary_name "aarch64-unknown-linux-musl") "tokenburn-zed"
}

def "test package: windows is zip with exe" [] {
    assert (is_windows "x86_64-pc-windows-msvc")
    assert equal (asset_name "x86_64-pc-windows-msvc") "tokenburn-zed-x86_64-pc-windows-msvc.zip"
    assert equal (binary_name "x86_64-pc-windows-msvc") "tokenburn-zed.exe"
}

def "test package: names agree with the extension source" [] {
    let root = ($env.CURRENT_FILE | path dirname | path join ".." "..")
    let src = (open --raw ($root | path join "src" "platform.rs"))
    for t in ["aarch64-apple-darwin" "x86_64-apple-darwin" "aarch64-unknown-linux-musl" "x86_64-unknown-linux-musl" "x86_64-pc-windows-msvc"] {
        assert ($src | str contains $"\"($t)\"") $"src/platform.rs maps a platform to ($t)"
    }
    assert ($src | str contains 'format!("tokenburn-zed-{target}.{ext}")')
}

def "test package: every release target is built by the workflow" [] {
    let root = ($env.CURRENT_FILE | path dirname | path join ".." "..")
    let wf = (open --raw ($root | path join ".github" "workflows" "release.yml"))
    for t in ["aarch64-apple-darwin" "x86_64-apple-darwin" "aarch64-unknown-linux-musl" "x86_64-unknown-linux-musl" "x86_64-pc-windows-msvc"] {
        assert ($wf | str contains $t) $"release.yml builds ($t)"
    }
}

def main [] { run-tests }
