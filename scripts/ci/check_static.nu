#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Assert a Linux binary is statically linked and runs
# ──────────────────────────────────────────────────────────────────────────────
# The musl release binaries must run on any distro: no dynamic loader, no libc.
#
# Usage: nu scripts/ci/check_static.nu <binary>
# ──────────────────────────────────────────────────────────────────────────────

# ldd's verdict on a static binary differs between glibc versions.
export def is_static [ldd_output: string] {
    $ldd_output =~ '(not a dynamic executable|statically linked)'
}

def main [binary: path] {
    let ldd = do { ^ldd $binary } | complete
    if not (is_static $"($ldd.stdout)($ldd.stderr)") {
        print $"❌ ($binary) is dynamically linked:\n($ldd.stdout)"
        exit 1
    }
    ^$binary --version
}
