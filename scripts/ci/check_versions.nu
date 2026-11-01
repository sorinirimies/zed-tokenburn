#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Versions agree
# ──────────────────────────────────────────────────────────────────────────────
# The workspace version (Cargo.toml), the versions of both extensions
# (extension.toml, mcp/extension.toml) and, for a release, the tag must all be the
# same — Zed's registry reads extension.toml, the downloaded binary comes from the
# tag's release.
#
# Usage:
#   nu scripts/ci/check_versions.nu            # Cargo.toml == both extension.toml
#   nu scripts/ci/check_versions.nu v0.2.0     # … and == the tag
# ──────────────────────────────────────────────────────────────────────────────

# The extension manifests whose version must follow Cargo.toml.
export const MANIFESTS = ["extension.toml" "mcp/extension.toml"]

# The mismatches between the versions (empty = all agree). `extensions` maps a
# manifest path to its version.
export def mismatches [cargo: string, extensions: record, tag?: string]: nothing -> list<string> {
    mut out = []
    for e in ($extensions | transpose file version) {
        if $cargo != $e.version {
            $out = ($out | append $"Cargo.toml has ($cargo) but ($e.file) has ($e.version)")
        }
    }
    if $tag != null {
        let v = ($tag | str replace --regex '^v' '')
        if $v != $cargo {
            $out = ($out | append $"tag ($tag) does not match Cargo.toml ($cargo)")
        }
    }
    $out
}

def main [tag?: string] {
    let cargo = (open Cargo.toml | get workspace.package.version)
    let exts = ($MANIFESTS | each {|f| {file: $f, version: (open $f | get version)} }
        | transpose --header-row --as-record)
    let bad = (mismatches $cargo $exts $tag)
    if ($bad | is-not-empty) {
        for b in $bad { print --stderr $"(ansi red)❌ ($b)(ansi reset)" }
        exit 1
    }
    print $"(ansi green)✅ versions agree: ($cargo)(ansi reset)"
}
