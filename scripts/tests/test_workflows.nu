#!/usr/bin/env nu
# ── TokenBurn for Zed · test_workflows.nu ────────────────────────────────────
# GitHub and Gitea run the same CI: every workflow exists on both hosts (except
# GitHub's Dependabot auto-merge and Gitea's actions-update, its Dependabot
# stand-in), the nightlies keep their schedules, the justfile can start each
# nightly on demand, and all scripting is Nushell.

use std/assert
use runner.nu *
use ../ci/deps_update.nu [LOCKFILES]

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
    assert equal $gh (workflows ".gitea" | where $it != "actions-update.yml")
}

def "test workflows: gitea bumps its actions nightly" [] {
    assert equal (cron ".gitea" "actions-update.yml") ["45 2 * * *"]
    assert (root | path join ".github" "dependabot.yml" | path exists) "GitHub uses Dependabot"
}

def "test workflows: dependabot covers cargo but never the pinned crates" [] {
    let updates = open (root | path join ".github" "dependabot.yml") | get updates
    assert equal ($updates | get package-ecosystem | sort) ["cargo" "github-actions"]
    let ignored = $updates | where package-ecosystem == "cargo" | first | get ignore | get dependency-name
    assert ("tokenburn-core" in $ignored) "tokenburn-core stays pinned (zstd)"
    assert ("zed_extension_api" in $ignored) "zed_extension_api bumps are releases"
}

def "test workflows: auto-merge waits for a green CI run" [] {
    let wf = open (root | path join ".github" "workflows" "auto-merge.yml")
    assert equal $wf.on.workflow_run.workflows ["CI"]
    let job = $wf.jobs.auto-merge
    assert ($job.if | str contains "conclusion == 'success'")
    assert ($job.if | str contains "dependabot[bot]")
    assert ($job.steps | any {|s| ($s.run? | default "") | str starts-with "nu scripts/ci/auto_merge.nu" })
}

def "test workflows: the nightlies are scheduled on both hosts" [] {
    for n in $NIGHTLIES {
        assert equal (cron ".github" $n.file) [$n.github] $"GitHub ($n.file)"
        assert equal (cron ".gitea" $n.file) [$n.gitea] $"Gitea ($n.file)"
    }
}

def "test workflows: deps updates cover the fuzz lockfile" [] {
    assert ("fuzz/Cargo.lock" in $LOCKFILES)
}

def "test workflows: deps updates run the full gate before pushing" [] {
    for host in [".github" ".gitea"] {
        let raw = (open --raw (root | path join $host "workflows" "deps-update.yml"))
        assert ($raw | str contains "nu scripts/ci/quality_gate.nu") $"($host) runs the gate"
        assert ($raw | str contains "nu scripts/ci/deps_update.nu update") $"($host) updates via deps_update.nu"
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
        assert ($raw | str contains "nu scripts/ci/fuzz_all.nu") $"($host) fuzzes all targets"
    }
}

# A run step longer than one line is shell logic. Allowed only to install the
# toolchain and Nushell itself (before nu exists); everything else is a nu script.
def "test workflows: run steps call nushell scripts" [] {
    for host in [".github" ".gitea"] {
        for f in (workflows $host) {
            let wf = open (root | path join $host "workflows" $f)
            for job in ($wf.jobs | values) {
                for s in ($job.steps? | default [] | where {|s| ($s.run? | default "" | str trim | lines | length) > 1 }) {
                    let name = $s.name? | default ""
                    assert ($name | str starts-with "Install") $"($host)/($f): '($name)' is shell logic, move it to scripts/*.nu"
                }
            }
        }
    }
}

def "test workflows: no shell or python scripts" [] {
    let found = glob ((root) | path join "**" "*.{sh,bash,zsh,py}") --exclude [**/target/** **/.git/**]
    assert equal $found [] "scripting is Nushell only"
}

def "test workflows: justfile recipes run in nushell" [] {
    let just = open --raw (root | path join "justfile")
    assert ($just | str contains 'set shell := ["nu", "-c"]')
    assert not ($just =~ '#!/usr/bin/env (sh|bash)') "no sh/bash shebang recipes"
}

def main [] { run-tests }

def "test workflows: the msrv job pins the toolchain from Cargo.toml without a version ref" [] {
    let cargo = open (root | path join Cargo.toml)
    let msrv = $cargo | get -o workspace.package.rust-version | default ($cargo | get -o package.rust-version)
    let step = open (root | path join .github workflows ci.yml) | get jobs.msrv.steps
        | where {|s| ($s | get -o uses | default "") | str starts-with "dtolnay/rust-toolchain" } | first
    # A version ref (@1.90) is something Dependabot bumps; @master + toolchain is not.
    assert equal $step.uses "dtolnay/rust-toolchain@master"
    assert equal ($step.with.toolchain | into string) $msrv
}

def "test workflows: rust toolchain refs are channels Dependabot leaves alone" [] {
    let refs = ls (root | path join .github workflows) | get name | each {|f| open --raw $f | decode utf-8 }
        | str join "\n" | parse --regex 'dtolnay/rust-toolchain@(?<ref>\S+)' | get ref | uniq
    assert equal ($refs | where $it not-in [stable beta nightly master]) []
}
