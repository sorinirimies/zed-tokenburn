#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Build both extensions exactly the way Zed does
# ──────────────────────────────────────────────────────────────────────────────
# Zed runs `cargo build --target wasm32-wasip2 --release --target-dir <dir>/target`
# in the extension's directory and loads `<dir>/target/wasm32-wasip2/release/
# <crate>.wasm`. This checks that works for the status-bar extension (repository
# root) and the MCP server extension (`mcp/`, which pulls shared code from `../src`).
#
# Usage: nu scripts/ci/zed_build.nu
# ──────────────────────────────────────────────────────────────────────────────

# The extensions: directory and wasm crate name.
export const EXTENSIONS = [[dir crate]; ["." "zed_tokenburn"] ["mcp" "zed_tokenburn_mcp"]]

# Where Zed expects the module of an extension.
export def wasm_path [dir: string, crate: string]: nothing -> string {
    $dir | path join "target" "wasm32-wasip2" "release" $"($crate).wasm"
}

def main [] {
    for e in $EXTENSIONS {
        let target = ($e.dir | path join "target" | path expand)
        cd $e.dir
        ^cargo build --target wasm32-wasip2 --release --target-dir $target
        cd -
        let wasm = (wasm_path $e.dir $e.crate)
        if not ($wasm | path exists) {
            print --stderr $"(ansi red)❌ ($wasm) was not built(ansi reset)"
            exit 1
        }
        print $"(ansi green)✅ ($wasm)(ansi reset)"
    }
}
