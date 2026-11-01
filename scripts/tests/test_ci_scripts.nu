#!/usr/bin/env nu
# ── TokenBurn for Zed · test_ci_scripts.nu ───────────────────────────────────
# The decisions the CI scripts make, without network, git or cargo.

use std/assert
use runner.nu *
use ../ci/deps_update.nu [present commit_message]
use ../ci/publish_crate.nu [already_published]
use ../ci/check_static.nu [is_static]
use ../ci/gitea_release.nu [release_payload]
use ../ci/actions_update.nu [uses major_of bump PINNED]
use ../ci/auto_merge.nu [parse_bump is_major decide]
use ../ci/quality_gate.nu [select_steps STEPS]

def "test quality_gate: runs every step by default in order" [] {
    assert equal (select_steps "" []) ($STEPS | get id)
}

def "test quality_gate: only and skip pick steps" [] {
    assert equal (select_steps "versions, fmt" []) ["fmt" "versions"] "gate order, not argument order"
    assert equal (select_steps "" ["test" "nu"] | where $it in ["test" "nu"]) []
    assert equal (select_steps "test" ["test"]) []
}

def "test quality_gate: unknown steps are an error" [] {
    assert error { select_steps "fmt,lint" [] }
}

def "test deps_update: commit message is a dated chore deps" [] {
    assert equal (commit_message "2026-01-02") "chore(deps): nightly cargo update 2026-01-02"
}

def "test deps_update: only existing lockfiles are touched" [] {
    let root = ($env.CURRENT_FILE | path dirname | path join ".." "..")
    cd $root
    assert equal (present ["Cargo.lock" "no/such/Cargo.lock"]) ["Cargo.lock"]
}

def "test publish_crate: a re-release counts as success" [] {
    assert (already_published "error: crate version `0.1.0` is already uploaded")
    assert (already_published "error: crate kv@0.1.0 already exists on crates.io index")
    assert not (already_published "error: failed to verify package tarball")
}

def "test check_static: both ldd wordings are static" [] {
    assert (is_static "\tnot a dynamic executable")
    assert (is_static "\tstatically linked")
    assert not (is_static "\tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6")
}

def "test gitea_release: payload carries tag, title and notes" [] {
    assert equal (release_payload "v1.2.3" "KV" "notes") { tag_name: "v1.2.3", name: "KV v1.2.3", body: "notes" }
}

def "test actions_update: finds every action and major" [] {
    let text = "steps:\n  - uses: actions/checkout@v4\n  - uses: Swatinem/rust-cache@v2\n  - uses: hustcer/setup-nu@v3\n"
    assert equal (uses $text | get repo) ["actions/checkout" "Swatinem/rust-cache" "hustcer/setup-nu"]
    assert equal (uses $text | get major) [4 2 3]
}

def "test actions_update: major of a release tag" [] {
    assert equal (major_of "v5.2.0") 5
    assert equal (major_of "v12") 12
    assert equal (major_of "release-1") null
}

def "test actions_update: bumps older majors only" [] {
    let text = "- uses: Swatinem/rust-cache@v2\n- uses: hustcer/setup-nu@v3\n- uses: foo/bar@v9\n"
    let out = bump $text { "Swatinem/rust-cache": 3, "hustcer/setup-nu": 3, "foo/bar": 1 }
    assert ($out | str contains "Swatinem/rust-cache@v3")
    assert ($out | str contains "hustcer/setup-nu@v3")
    assert ($out | str contains "foo/bar@v9")
}

def "test actions_update: pinned actions never move" [] {
    let text = "- uses: actions/checkout@v4\n"
    assert equal (bump $text { "actions/checkout": 6 }) $text
    assert ("actions/upload-artifact" in $PINNED)
}

def "test actions_update: v1 does not touch v10" [] {
    let out = bump "- uses: a/b@v1\n- uses: a/b@v10\n" { "a/b": 2 }
    assert equal $out "- uses: a/b@v2\n- uses: a/b@v10\n"
}

def pr [title: string, --author: string = "app/dependabot", --state: string = "OPEN", --sha: string = "abc"] {
    {author: {login: $author}, state: $state, headRefOid: $sha, title: $title, number: 7}
}

def "test auto_merge: parses single bumps and not groups" [] {
    assert equal (parse_bump "ci(deps): bump dependabot/fetch-metadata from 2 to 3") {from: "2", to: "3"}
    assert equal (parse_bump "chore(deps): bump serde from 1.0.210 to v1.0.211") {from: "1.0.210", to: "1.0.211"}
    assert equal (parse_bump "chore(deps): bump the cargo group across 2 directories with 4 updates") null
}

def "test auto_merge: only a changed first component is major" [] {
    assert (is_major "2" "3")
    assert (is_major "1.9.0" "2.0.0")
    assert not (is_major "1.0.1" "1.4.0")
    assert not (is_major "0.3.7" "0.4.0")
}

def "test auto_merge: merges green minors and groups" [] {
    assert (decide (pr "bump x from 1.2.0 to 1.3.0") "abc").merge
    assert (decide (pr "bump x from 0.3.0 to 0.4.0") "abc").merge
    assert (decide (pr "bump the cargo group with 3 updates") "abc").merge
}

def "test auto_merge: leaves majors and anything unexpected alone" [] {
    assert not (decide (pr "bump x from 1.2.0 to 2.0.0") "abc").merge
    assert not (decide (pr "bump x from 1.2.0 to 1.3.0" --author "sorinirimies") "abc").merge
    assert not (decide (pr "bump x from 1.2.0 to 1.3.0" --state "MERGED") "abc").merge
    assert not (decide (pr "bump x from 1.2.0 to 1.3.0" --sha "new") "abc").merge "branch moved after CI"
}

def main [] { run-tests }
