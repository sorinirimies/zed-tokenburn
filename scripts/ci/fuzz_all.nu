#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Fuzz every cargo-fuzz target for a while (nightly toolchain)
# ──────────────────────────────────────────────────────────────────────────────
# New targets are picked up automatically (cargo fuzz list). Stops at the first
# crash; the reproducer is in fuzz/artifacts/<target>/.
#
# Usage: nu scripts/ci/fuzz_all.nu [--secs 60]
# ──────────────────────────────────────────────────────────────────────────────

export def fuzz_args [target: string, secs: int] {
    [+nightly fuzz run $target -- $"-max_total_time=($secs)"]
}

def main [--secs: int = 60] {
    let targets = ^cargo +nightly fuzz list | lines | where {|l| $l | is-not-empty }
    if ($targets | is-empty) { print "no fuzz targets"; exit 1 }
    for t in $targets {
        print $"(ansi cyan)▶(ansi reset) ($t) for ($secs)s"
        ^cargo ...(fuzz_args $t $secs)
    }
}
