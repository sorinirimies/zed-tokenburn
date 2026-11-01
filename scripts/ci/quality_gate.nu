#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Quality gate
# ──────────────────────────────────────────────────────────────────────────────
# The same sequence locally (`just check-all`), in CI, in the nightly dependency
# update and before a bump:
#   1. cargo fmt --check
#   2. cargo clippy (workspace, all targets, -D warnings)
#   3. cargo test --workspace
#   4. cargo doc with warnings as errors (broken intra-doc links fail docs.rs)
#   5. versions agree (Cargo.toml == EXTRA_MANIFESTS)
#   6. nushell script tests
# Project-specific: clippy for wasm32-wasip2 and the Zed-style build of both
# extensions (scripts/ci/zed_build.nu). One place, so every caller gets them.
#
# Usage: nu scripts/ci/quality_gate.nu [--only fmt,clippy,…] [--skip-fmt] [--skip-test] [--skip-wasm] [--skip-nu]
# ──────────────────────────────────────────────────────────────────────────────

# The gate's steps, in order. `just check-all` runs them all; CI runs them as
# separate parallel jobs (`--only fmt`, `--only clippy,wasm-clippy`, …), and a
# test checks that the CI jobs cover every step on both hosts.
export const STEPS = [
    [id label];
    [fmt "cargo fmt --all -- --check"]
    [clippy "cargo clippy --workspace --all-targets -- -D warnings"]
    [wasm-clippy "cargo clippy --target wasm32-wasip2 (extensions)"]
    [zed-build "Zed-style build of both extensions"]
    [test "cargo test --workspace"]
    [doc "cargo doc (warnings are errors)"]
    [versions "versions agree"]
    [nu "nushell tests"]
]

# The step ids to run: `only` ("" = all, else comma-separated ids) minus `skip`.
export def select_steps [only: string, skip: list<string>]: nothing -> list<string> {
    let all = $STEPS | get id
    let wanted = if ($only | str trim | is-empty) { $all } else { $only | split row "," | str trim }
    let unknown = $wanted | where $it not-in $all
    if ($unknown | is-not-empty) {
        error make {msg: $"unknown gate steps: ($unknown | str join ', ') \(known: ($all | str join ', '))"}
    }
    $all | where {|s| $s in $wanted and $s not-in $skip }
}

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

def step_cmd [id: string]: nothing -> closure {
    match $id {
        "fmt" => {|| cargo fmt --all -- --check }
        "clippy" => {|| cargo clippy --workspace --all-targets --all-features -- -D warnings }
        "wasm-clippy" => {|| cargo clippy -p zed-tokenburn -p zed-tokenburn-mcp --target wasm32-wasip2 -- -D warnings }
        "zed-build" => {|| nu scripts/ci/zed_build.nu }
        "test" => {|| cargo test --workspace --all-features }
        "doc" => {|| with-env { RUSTDOCFLAGS: "-D warnings" } { cargo doc --no-deps --workspace --quiet } }
        "versions" => {|| nu scripts/ci/check_versions.nu }
        "nu" => {|| nu scripts/tests/run_all.nu }
    }
}

def main [
    --only: string = ""   # comma-separated step ids (see STEPS); default: all
    --skip-fmt
    --skip-test
    --skip-wasm           # skips wasm-clippy and zed-build
    --skip-nu
] {
    let skip = (
        (if $skip_fmt { ["fmt"] } else { [] })
        ++ (if $skip_test { ["test"] } else { [] })
        ++ (if $skip_wasm { ["wasm-clippy" "zed-build"] } else { [] })
        ++ (if $skip_nu { ["nu"] } else { [] })
    )
    let ids = select_steps $only $skip
    print $"(ansi cyan)══ TokenBurn for Zed — Quality Gate ══(ansi reset)"
    mut ok = true
    for id in $ids {
        let label = $STEPS | where id == $id | first | get label
        $ok = (run_step $label (step_cmd $id)) and $ok
    }
    if not $ok {
        print $"(ansi red)✗ Quality gate FAILED(ansi reset)"
        exit 1
    }
    print $"(ansi green)✔ Quality gate passed(ansi reset)"
}
