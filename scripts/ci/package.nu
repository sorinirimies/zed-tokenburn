#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Package a release binary
# ──────────────────────────────────────────────────────────────────────────────
# Packs target/<target>/release/tokenburn-zed[.exe] into
#   dist/tokenburn-zed-<target>.tar.gz   (zip on Windows)
# with the binary at the archive root — exactly what the extension downloads
# (src/platform.rs: asset_name / binary_path) — plus a .sha256 next to it.
#
# Usage:
#   nu scripts/ci/package.nu <target> [--out dist]
# ──────────────────────────────────────────────────────────────────────────────

export def is_windows [target: string]: nothing -> bool {
    $target | str contains "windows"
}

# Archive file name for a target (must match src/platform.rs).
export def asset_name [target: string]: nothing -> string {
    let ext = if (is_windows $target) { "zip" } else { "tar.gz" }
    $"tokenburn-zed-($target).($ext)"
}

# Binary file name for a target.
export def binary_name [target: string]: nothing -> string {
    if (is_windows $target) { "tokenburn-zed.exe" } else { "tokenburn-zed" }
}

def main [target: string, --out: string = "dist"] {
    let bin = (binary_name $target)
    let src = ($"target/($target)/release/($bin)")
    if not ($src | path exists) {
        print --stderr $"(ansi red)❌ ($src) not found — build it first:(ansi reset) cargo build --release -p tokenburn-zed --target ($target)"
        exit 1
    }
    mkdir $out
    let stage = (mktemp -d)
    cp $src $stage
    let asset = (asset_name $target)
    let dest = ($out | path join $asset | path expand)
    if (is_windows $target) {
        if (which 7z | is-not-empty) {
            do { cd $stage; ^7z a -tzip $dest $bin } | ignore
        } else if (which zip | is-not-empty) {
            do { cd $stage; ^zip -q $dest $bin }
        } else {
            # Windows runners: PowerShell is always there.
            ^pwsh -NoProfile -Command $"Compress-Archive -Path '($stage | path join $bin)' -DestinationPath '($dest)' -Force"
        }
    } else {
        ^tar -C $stage -czf $dest $bin
    }
    rm -rf $stage
    let sum = (open --raw $dest | hash sha256)
    $"($sum)  ($asset)\n" | save --force $"($dest).sha256"
    print $"(ansi green)✅ ($dest)(ansi reset)"
}
