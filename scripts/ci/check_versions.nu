#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Versions agree
# ──────────────────────────────────────────────────────────────────────────────
# The workspace version (Cargo.toml), the extension version (extension.toml) and,
# for a release, the tag must all be the same — Zed's registry reads
# extension.toml, the downloaded binary comes from the tag's release.
#
# Usage:
#   nu scripts/ci/check_versions.nu            # Cargo.toml == extension.toml
#   nu scripts/ci/check_versions.nu v0.2.0     # … and == the tag
# ──────────────────────────────────────────────────────────────────────────────

# The mismatches between the versions (empty = all agree).
export def mismatches [cargo: string, extension: string, tag?: string]: nothing -> list<string> {
    mut out = []
    if $cargo != $extension {
        $out = ($out | append $"Cargo.toml has ($cargo) but extension.toml has ($extension)")
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
    let ext = (open extension.toml | get version)
    let bad = (mismatches $cargo $ext $tag)
    if ($bad | is-not-empty) {
        for b in $bad { print --stderr $"(ansi red)❌ ($b)(ansi reset)" }
        exit 1
    }
    print $"(ansi green)✅ versions agree: ($cargo)(ansi reset)"
}
