#!/usr/bin/env nu
# ── TokenBurn for Zed · test_workflows.nu ────────────────────────────────────
# GitHub and Gitea run the same CI: every workflow exists on both hosts (except
# the GitHub-only Dependabot auto-merge), the nightlies keep their schedules, and
# the justfile can start each nightly on demand.

use std/assert
use runner.nu *

def root [] { $env.CURRENT_FILE | path dirname | path join ".." ".." }

def workflows [host: string]: nothing -> list<string> {
    ls (root | path join $host "workflows") | get name | path basename | sort
}

def cron [host: string, file: string]: nothing -> list<string> {
    open (root | path join $host "workflows" $file) | get on | get -o schedule | default [] | get cron
}

# The nightly workflows and their UTC schedule on [GitHub, Gitea].
const NIGHTLIES = [[file github gitea];
    ["deps-update.yml" "0 2 * * *" "30 2 * * *"]
    ["security.yml" "0 3 * * *" "0 3 * * *"]
    ["fuzz.yml" "0 4 * * *" "0 4 * * *"]]

def "test workflows: both hosts have the same workflows" [] {
    let gh = (workflows ".github" | where $it != "auto-merge.yml")
    assert equal $gh (workflows ".gitea")
}

def "test workflows: the nightlies are scheduled on both hosts" [] {
    for n in $NIGHTLIES {
        assert equal (cron ".github" $n.file) [$n.github] $"GitHub ($n.file)"
        assert equal (cron ".gitea" $n.file) [$n.gitea] $"Gitea ($n.file)"
    }
}

def "test workflows: deps updates run the full gate before pushing" [] {
    for host in [".github" ".gitea"] {
        let raw = (open --raw (root | path join $host "workflows" "deps-update.yml"))
        assert ($raw | str contains "nu scripts/ci/quality_gate.nu") $"($host) runs the gate"
        assert ($raw | str contains "fuzz/Cargo.lock") $"($host) updates the fuzz lockfile"
        assert ($raw | str contains "cancel-in-progress: false") $"($host) never cancels mid-push"
    }
}

def "test workflows: every nightly can be started from the justfile" [] {
    let just = (open --raw (root | path join "justfile"))
    for r in ["deps-update-now:" "security-now:" "fuzz-now:" "gh-run workflow" "gitea-run workflow"] {
        assert ($just | str contains $r) $"justfile has ($r)"
    }
}

def "test workflows: the fuzz workflow fuzzes every target" [] {
    let targets = (ls (root | path join "fuzz" "fuzz_targets") | get name | path parse | get stem)
    let manifest = (open (root | path join "fuzz" "Cargo.toml") | get bin | get name | sort)
    assert equal $manifest ($targets | sort) "every fuzz_targets/*.rs is a [[bin]]"
    for host in [".github" ".gitea"] {
        let raw = (open --raw (root | path join $host "workflows" "fuzz.yml"))
        assert ($raw | str contains "cargo +nightly fuzz list") $"($host) iterates all targets"
    }
}

def main [] { run-tests }
