#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — merge a Dependabot PR once CI is green (used by auto-merge.yml)
# ──────────────────────────────────────────────────────────────────────────────
#   nu scripts/ci/auto_merge.nu <branch> <sha>   merge the open Dependabot PR on
#                                                <branch> if its head is still <sha>
#
# auto-merge.yml runs after CI succeeded on a Dependabot PR, so "it builds" is
# already proven. Majors (1.x → 2.x) wait for a human; everything else — patch,
# minor, 0.x bumps, grouped updates — is squash-merged. Prints merged=true|false.
# ──────────────────────────────────────────────────────────────────────────────

const DEPENDABOT = "app/dependabot"

# `{from, to}` of a single-dependency title ("bump serde from 1.0.1 to 1.0.2"),
# or null for a grouped update ("bump the cargo group … with 3 updates").
export def parse_bump [title: string] {
    let m = $title | parse --regex 'from v?(?<from>\d[\w.+-]*) to v?(?<to>\d[\w.+-]*)'
    if ($m | is-empty) { null } else { $m | first }
}

# A semver major: the first version component changes (0.3 → 0.4 is not one).
export def is_major [from: string, to: string] {
    let major = {|v| $v | split row "." | first | into int }
    (do $major $from) != (do $major $to)
}

# Whether to merge `pr` (from `gh pr list --json author,headRefOid,state,title`)
# whose CI run was for `sha`: `{merge: bool, reason: string}`.
export def decide [pr: record, sha: string] {
    if $pr.author.login != $DEPENDABOT {
        return {merge: false, reason: $"not a Dependabot PR \(($pr.author.login))"}
    }
    if $pr.state != "OPEN" { return {merge: false, reason: $"PR is ($pr.state)"} }
    if $pr.headRefOid != $sha {
        return {merge: false, reason: "the branch moved since CI ran; the next CI run decides"}
    }
    let bump = parse_bump $pr.title
    if $bump == null { return {merge: true, reason: "grouped minor/patch update"} }
    if (is_major $bump.from $bump.to) {
        return {merge: false, reason: $"major bump ($bump.from) → ($bump.to): needs a human"}
    }
    {merge: true, reason: $"($bump.from) → ($bump.to)"}
}

def main [branch: string, sha: string] {
    let prs = ^gh pr list --head $branch --state open --json number,author,headRefOid,state,title
        | from json
    if ($prs | is-empty) {
        print -e $"no open PR for ($branch)"
        print "merged=false"
        return
    }
    let pr = $prs | first
    let d = decide $pr $sha
    print -e $"#($pr.number) ($pr.title): ($d.reason)"
    if $d.merge { ^gh pr merge $pr.number --squash --delete-branch }
    print $"merged=($d.merge)"
}
