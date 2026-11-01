#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Quality gate
# ──────────────────────────────────────────────────────────────────────────────
# The same sequence locally (`just check-all`), in CI and before a bump:
#   1. cargo fmt --check
#   2. cargo clippy (workspace, native) and clippy of the extension for wasm32-wasip2
#   3. cargo test --workspace
#   4. both extensions build for wasm32-wasip2, also exactly the way Zed builds them
#   5. versions agree (Cargo.toml == extension.toml)
#   6. nushell script tests
#
# Usage: nu scripts/ci/quality_gate.nu [--skip-fmt] [--skip-test] [--skip-wasm] [--skip-nu]
# ──────────────────────────────────────────────────────────────────────────────

def run_step [label: string, cmd: closure]: nothing -> bool {
    print $"(ansi cyan)▶(ansi reset) ($label)"
    let r = (do $cmd | complete)
    if $r.exit_code == 0 {
        print $"(ansi green)  ✔ ok(ansi reset)"
        true
    } else {
        print $"(ansi red)  ✗ failed(ansi reset)"
        if ($r.stdout | str trim | is-not-empty) { print $r.stdout }
        if ($r.stderr | str trim | is-not-empty) { print $r.stderr }
        false
    }
}

def main [--skip-fmt, --skip-test, --skip-wasm, --skip-nu] {
    print $"(ansi cyan)══ TokenBurn for Zed — Quality Gate ══(ansi reset)"
    mut ok = true
    if not $skip_fmt {
        $ok = (run_step "cargo fmt --all -- --check" { cargo fmt --all -- --check }) and $ok
    }
    $ok = (run_step "cargo clippy --workspace --all-targets -- -D warnings" {
        cargo clippy --workspace --all-targets --all-features -- -D warnings
    }) and $ok
    if not $skip_wasm {
        $ok = (run_step "cargo clippy --target wasm32-wasip2 (extension)" {
            cargo clippy -p zed-tokenburn -p zed-tokenburn-mcp --target wasm32-wasip2 -- -D warnings
        }) and $ok
        $ok = (run_step "cargo build --target wasm32-wasip2 --release (extension)" {
            cargo build -p zed-tokenburn -p zed-tokenburn-mcp --target wasm32-wasip2 --release
        }) and $ok
        $ok = (run_step "Zed-style build of both extensions" { nu scripts/ci/zed_build.nu }) and $ok
    }
    if not $skip_test {
        $ok = (run_step "cargo test --workspace" { cargo test --workspace --all-features }) and $ok
    }
    $ok = (run_step "versions agree" { nu scripts/ci/check_versions.nu }) and $ok
    if not $skip_nu {
        $ok = (run_step "nushell tests" { nu scripts/tests/run_all.nu }) and $ok
    }
    if not $ok {
        print $"(ansi red)✗ Quality gate FAILED(ansi reset)"
        exit 1
    }
    print $"(ansi green)✔ Quality gate passed(ansi reset)"
}
