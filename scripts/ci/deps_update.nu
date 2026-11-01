#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Nightly dependency update helpers (used by deps-update.yml)
# ──────────────────────────────────────────────────────────────────────────────
#   nu scripts/ci/deps_update.nu sync               reset to origin/main (scheduled runs start stale)
#   nu scripts/ci/deps_update.nu update             cargo update here and in fuzz/
#   nu scripts/ci/deps_update.nu changed            prints changed=true|false (→ $GITHUB_OUTPUT)
#   nu scripts/ci/deps_update.nu commit <bot> <mail> commit the lockfiles, push to main
# ──────────────────────────────────────────────────────────────────────────────

export const LOCKFILES = ["Cargo.lock" "fuzz/Cargo.lock"]

# The lockfiles that exist in this checkout (fuzz/ is optional).
export def present [files: list<string>] {
    $files | where {|f| $f | path exists }
}

export def commit_message [date: string] {
    $"chore\(deps\): nightly cargo update ($date)"
}

def "main sync" [] {
    ^git fetch origin main
    ^git checkout -B main origin/main
    ^git reset --hard origin/main
    ^git log -1 --oneline
}

def "main update" [] {
    ^cargo update
    if ("fuzz/Cargo.toml" | path exists) { do { cd fuzz; ^cargo update } }
}

def "main changed" [] {
    let files = present $LOCKFILES
    let r = do { ^git diff --quiet -- ...$files } | complete
    let changed = $r.exit_code != 0
    # stdout is $GITHUB_OUTPUT: key=value lines only; the human part goes to stderr.
    if $changed { print -e (^git diff --stat -- ...$files) }
    print $"changed=($changed)"
}

def "main commit" [bot: string, email: string] {
    ^git config user.name $bot
    ^git config user.email $email
    ^git add ...(present $LOCKFILES)
    ^git commit -m (commit_message (date now | date to-timezone UTC | format date "%Y-%m-%d"))
    ^git push origin HEAD:main
}

def main [] { print "usage: nu scripts/ci/deps_update.nu (sync | update | changed | commit <bot> <email>)" }
