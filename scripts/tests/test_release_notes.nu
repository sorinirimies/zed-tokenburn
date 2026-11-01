#!/usr/bin/env nu
# ── TokenBurn for Zed · test_release_notes.nu ────────────────────────────────

use std/assert
use runner.nu *
use ../ci/release_notes.nu [notes]

def "test notes: heading install and changes" [] {
    let n = (notes "0.2.0" "### ✨ Features\n\n- A thing")
    assert ($n | str starts-with "# TokenBurn for Zed 0.2.0")
    assert ($n | str contains "cargo install tokenburn-zed")
    assert ($n | str contains "## Changes")
    assert ($n | str contains "- A thing")
}

def main [] { run-tests }
