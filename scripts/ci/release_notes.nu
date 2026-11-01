#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Release notes
# ──────────────────────────────────────────────────────────────────────────────
# Writes RELEASE_NOTES.md for a tag: install instructions plus this release's
# section of the changelog (git-cliff --latest; the workflow runs at the tag, so
# --unreleased would be empty).
#
# Usage:
#   nu scripts/ci/release_notes.nu v0.2.0
# ──────────────────────────────────────────────────────────────────────────────

# The notes text for `version` with the changelog section `changes`.
export def notes [version: string, changes: string]: nothing -> string {
    [
        $"# TokenBurn for Zed ($version)"
        ""
        "Install **TokenBurn** from Zed's extension gallery \(`zed: extensions`\); the"
        "extension downloads the matching `tokenburn-zed` binary from this release."
        "Or build it yourself and put it on `PATH`:"
        ""
        "```sh"
        "cargo install tokenburn-zed"
        "```"
        ""
        "## Changes"
        ""
        $changes
        ""
    ] | str join "\n"
}

def main [raw_tag: string] {
    let version = ($raw_tag | str replace --regex '^v' '')
    let changes = if (which git-cliff | is-not-empty) {
        let r = (do { ^git-cliff --latest --strip header } | complete)
        if $r.exit_code == 0 and ($r.stdout | str trim | is-not-empty) { $r.stdout | str trim } else { "- See the commit history." }
    } else {
        "- See the commit history."
    }
    notes $version $changes | save --force RELEASE_NOTES.md
    print $"(ansi green)✅ RELEASE_NOTES.md for ($version)(ansi reset)"
}
