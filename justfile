# TokenBurn for Zed — task runner
# Install just:      cargo install just
# Install git-cliff: cargo install git-cliff
# Install nu:        cargo install nu   (or https://www.nushell.sh)
# Usage: just <task>

# ── Default ───────────────────────────────────────────────────────────────────

default:
    @just --list

# ── Tool checks ───────────────────────────────────────────────────────────────

_check-git-cliff:
    @command -v git-cliff >/dev/null 2>&1 || { \
        echo "❌ git-cliff not found. Install with: cargo install git-cliff"; exit 1; \
    }

_check-nu:
    @command -v nu >/dev/null 2>&1 || { \
        echo "❌ nu (nushell) not found. Install: https://www.nushell.sh"; exit 1; \
    }

# Install the development tools (git-cliff, nu, the wasm target, cargo-audit/deny/fuzz)
install-tools:
    rustup target add wasm32-wasip2
    @command -v git-cliff >/dev/null 2>&1 || cargo install git-cliff --locked
    @command -v nu >/dev/null 2>&1 || cargo install nu --locked
    @command -v cargo-audit >/dev/null 2>&1 || cargo install cargo-audit --locked
    @command -v cargo-deny >/dev/null 2>&1 || cargo install cargo-deny --locked
    @command -v cargo-fuzz >/dev/null 2>&1 || cargo install cargo-fuzz --locked
    @echo "✅ Tools installed."

# ── Build ─────────────────────────────────────────────────────────────────────

# Check without building
check:
    cargo check --workspace

# Build the native server (dev)
build:
    cargo build -p tokenburn-zed

# Build the Zed extension to WebAssembly (what Zed does on install)
build-ext:
    cargo build -p zed-tokenburn -p zed-tokenburn-mcp --target wasm32-wasip2 --release

# Release build of the native server
build-release:
    cargo build --release -p tokenburn-zed

# Install the server on PATH — the extension prefers it over a download
install-server:
    cargo install --path crates/tokenburn-zed --locked --force

# ── Run ───────────────────────────────────────────────────────────────────────

# Print the status-bar line once (e.g. just status --period week --tool pi)
status *args:
    cargo run -q -p tokenburn-zed -- status {{ args }}

# Print the full markdown report
report *args:
    cargo run -q -p tokenburn-zed -- report {{ args }}

# Open tokenburn's desktop (gui) or web dashboard (e.g. just dashboard web --period month)
dashboard kind="gui" *args:
    cargo run -q -p tokenburn-zed -- dashboard {{ kind }} {{ args }}

# Install tokenburn's desktop and web dashboards (what the dashboard code actions start)
install-dashboards:
    cargo install tokenburn-gui tokenburn-web --locked

# Run the MCP server on stdin/stdout
mcp:
    cargo run -q -p tokenburn-zed -- mcp

# Try it in Zed: install the server, then "zed: install dev extension" on this folder
dev: install-server build-ext
    @echo "✅ Now in Zed: command palette → 'zed: install dev extension' → pick $(pwd)"
    @echo "   Open any file; the status bar shows 'TokenBurn: 🔥 …'."
    @echo "   For the agent tools, install $(pwd)/mcp too and set"
    @echo "   context_servers.tokenburn.settings.binary_path to $(command -v tokenburn-zed || echo ~/.cargo/bin/tokenburn-zed)"

# ── Test & quality ────────────────────────────────────────────────────────────

# Rust tests (unit + stdio end-to-end)
test:
    cargo test --workspace --all-features

# Nushell script tests
test-nu: _check-nu
    nu scripts/tests/run_all.nu

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all -- --check

# Clippy, native and for the extension's wasm target
clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    cargo clippy -p zed-tokenburn -p zed-tokenburn-mcp --target wasm32-wasip2 -- -D warnings

# Rust and Nu tests
test-all-nu: test test-nu
    @echo "✅ All Rust and Nu tests passed!"

# Line coverage (needs cargo-llvm-cov); writes target/llvm-cov/html
coverage:
    cargo llvm-cov --workspace --html
    @echo "→ target/llvm-cov/html/index.html"

# Docs for the workspace (no browser)
doc:
    cargo doc --no-deps --workspace

# Everything CI runs (fmt check, clippy, wasm build, tests, versions, nu tests)
check-all: fmt _check-nu
    nu scripts/ci/quality_gate.nu

# Security: known vulnerabilities (cargo install cargo-audit)
audit:
    cargo audit

# Security: licenses, advisories, bans, sources (cargo install cargo-deny)
deny:
    cargo deny check

# Fuzz one target (e.g. just fuzz mcp_message 60); needs nightly + cargo-fuzz
fuzz target="mcp_message" secs="60":
    cargo +nightly fuzz run {{ target }} -- -max_total_time={{ secs }}

# Every fuzz target for a short while
fuzz-all secs="30":
    for t in $(cargo +nightly fuzz list); do cargo +nightly fuzz run "$t" -- -max_total_time={{ secs }} || exit 1; done

# List the fuzz targets
fuzz-list:
    cargo +nightly fuzz list

# Security + fuzz locally, like the nightly workflows
nightly-local: audit deny fuzz-all

# ── Dependencies ──────────────────────────────────────────────────────────────

# Update Cargo.lock and fuzz/Cargo.lock (semver-compatible only)
update:
    cargo update
    cd fuzz && cargo update

# Show outdated dependencies (needs cargo-outdated)
outdated:
    cargo outdated --workspace

# What the nightly workflow does: update, full gate, commit and push if green
update-deps: update check-all
    git add Cargo.lock fuzz/Cargo.lock
    git diff --cached --quiet || git commit -m "chore(deps): update dependencies"
    git push origin "$(git branch --show-current)"
    @echo "✅ Dependency updates pushed to GitHub."

# ── Nightly workflows on demand ───────────────────────────────────────────────

# Run a GitHub workflow now (deps-update | security | fuzz | ci), needs gh
gh-run workflow="deps-update":
    gh workflow run {{ workflow }}.yml
    @echo "✅ Dispatched {{ workflow }} — gh run watch"

# Run a Gitea workflow now (deps-update | security | fuzz); needs GITEA_URL + GITEA_TOKEN
gitea-run workflow="deps-update":
    #!/usr/bin/env sh
    set -e
    : "${GITEA_URL:?set GITEA_URL, e.g. http://192.168.1.44:3000}"
    : "${GITEA_TOKEN:?set GITEA_TOKEN (a Gitea access token with repo scope)}"
    curl -sf -X POST "$GITEA_URL/api/v1/repos/sorin/zed-tokenburn/actions/workflows/{{ workflow }}.yml/dispatches" \
        -H "Authorization: token $GITEA_TOKEN" -H "Content-Type: application/json" -d '{"ref":"main"}'
    echo "✅ Dispatched {{ workflow }} on Gitea"

# The nightly dependency update, now, on both hosts
deps-update-now: (gh-run "deps-update")
    -just gitea-run deps-update

# Security audit workflow, now, on both hosts
security-now: (gh-run "security")
    -just gitea-run security

# Fuzz workflow, now, on both hosts
fuzz-now: (gh-run "fuzz")
    -just gitea-run fuzz

# ── Changelog ─────────────────────────────────────────────────────────────────

# Regenerate CHANGELOG.md from all tags
changelog: _check-git-cliff
    git-cliff --output CHANGELOG.md

# Preview the next release's changelog
changelog-preview: _check-git-cliff
    @git-cliff --unreleased

# Prepend only the unreleased commits to CHANGELOG.md
changelog-unreleased: _check-git-cliff
    git-cliff --unreleased --prepend CHANGELOG.md

# Show the latest release's entry
changelog-latest: _check-git-cliff
    @git-cliff --latest

# ── Version & release ─────────────────────────────────────────────────────────

# Current version (Cargo.toml; extension.toml must agree)
version: _check-nu
    @nu scripts/version.nu

# Check Cargo.toml and extension.toml agree (and a tag, if given)
check-versions tag="": _check-nu
    nu scripts/ci/check_versions.nu {{ tag }}

# Validate that a version gives a valid vX.Y.Z tag
validate-tag version: _check-nu
    nu scripts/ci/validate_tag.nu "v{{ version }}"

# What would be released, without changing anything
release-preview: _check-git-cliff _check-nu
    @echo "Current version: $(nu scripts/version.nu)"
    @echo ""
    @git-cliff --unreleased
    @echo "Ships: tokenburn-zed (crates.io + 5 binaries), extensions tokenburn + tokenburn-mcp-server"

# Everything in check-all plus a release build of the native binary
check-release: check-all build-release
    @echo "✅ Release gate passed"

# Dry-run the crates.io publish of the native binary
publish-dry:
    cargo publish --dry-run -p tokenburn-zed

# Bump Cargo.toml + both extension.toml + Cargo.lock, regenerate the changelog, commit and tag
bump version: _check-nu _check-git-cliff check-all
    nu scripts/bump_version.nu --yes --skip-checks {{ version }}

# Bump, tag and push to GitHub — starts the Release workflow
release version: (bump version)
    git push --follow-tags origin "$(git branch --show-current)"
    @echo "✅ v{{ version }} pushed — the Release workflow builds the binaries."

# First release of the version already in Cargo.toml (no bump)
release-initial: _check-nu _check-git-cliff check-all
    #!/usr/bin/env sh
    set -e
    version=$(nu scripts/version.nu); tag="v$version"
    nu scripts/ci/check_versions.nu "$tag"
    if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then echo "❌ $tag exists"; exit 1; fi
    if [ -n "$(git status --porcelain)" ]; then echo "❌ dirty tree"; exit 1; fi
    git-cliff --output CHANGELOG.md --tag "$tag"
    git add CHANGELOG.md
    git diff --cached --quiet || git commit -m "chore: release $tag"
    git tag -a "$tag" -m "Release $tag"
    git push --follow-tags origin "$(git branch --show-current)"

# Bump, tag and push to every remote — starts the Release workflows on both hosts
release-all version: (bump version) push-all

# Bump, tag and push to Gitea only
release-gitea-starscream version: (bump version)
    git push --follow-tags gitea-starscream "$(git branch --show-current)"

# First release, pushed to every remote
release-initial-all: release-initial
    -git push --follow-tags gitea-starscream "$(git branch --show-current)"

# Re-run the GitHub Release workflow for an existing tag (e.g. just release-retrigger 0.1.0)
release-retrigger version:
    gh workflow run release.yml --field tag=v{{ version }}

# Package a release binary locally (e.g. just package aarch64-apple-darwin)
package target:
    cargo build --release --locked -p tokenburn-zed --target {{ target }}
    nu scripts/ci/package.nu {{ target }} --out dist

# ── Git remotes ───────────────────────────────────────────────────────────────

REMOTES := "origin gitea-starscream"

# Push the current branch to every configured remote (continues on failure)
push-all:
    #!/usr/bin/env sh
    failed=""
    for r in {{ REMOTES }}; do
        git remote get-url "$r" >/dev/null 2>&1 || { echo "↷ skipping $r (not configured)"; continue; }
        git push --follow-tags "$r" "$(git branch --show-current)" || failed="$failed $r"
    done
    [ -z "$failed" ] || { echo "⚠️  Failed:$failed"; exit 1; }

# Show all configured remotes
remotes:
    @git remote -v

# Push the current branch to GitHub
push:
    git push origin "$(git branch --show-current)"

# Push the current branch to Gitea Starscream
push-gitea-starscream:
    git push gitea-starscream "$(git branch --show-current)"

# Push all tags to GitHub
push-tags:
    git push origin --tags

# Push all tags to every remote (continues on failure)
push-tags-all:
    #!/usr/bin/env sh
    failed=""
    for r in {{ REMOTES }}; do
        git remote get-url "$r" >/dev/null 2>&1 || { echo "↷ skipping $r (not configured)"; continue; }
        git push "$r" --tags || failed="$failed $r"
    done
    [ -z "$failed" ] || { echo "⚠️  Failed:$failed"; exit 1; }

# Latest commit + tags to every remote, after the gate (no bump)
push-release-all: check-all push-all

# Force-push the current branch to every remote (no lease — be sure!)
push-all-force:
    #!/usr/bin/env sh
    failed=""
    for r in {{ REMOTES }}; do
        git remote get-url "$r" >/dev/null 2>&1 || { echo "↷ skipping $r (not configured)"; continue; }
        git push --force "$r" "$(git branch --show-current)" || failed="$failed $r"
    done
    [ -z "$failed" ] || { echo "⚠️  Failed:$failed"; exit 1; }

# Pull the current branch from GitHub
pull:
    git pull origin "$(git branch --show-current)"

# Pull the current branch from Gitea Starscream
pull-gitea-starscream:
    git pull gitea-starscream "$(git branch --show-current)"

# Pull the current branch from every remote (continues on failure)
pull-all:
    #!/usr/bin/env sh
    failed=""
    for r in {{ REMOTES }}; do
        git remote get-url "$r" >/dev/null 2>&1 || { echo "↷ skipping $r (not configured)"; continue; }
        git pull "$r" "$(git branch --show-current)" || failed="$failed $r"
    done
    [ -z "$failed" ] || { echo "⚠️  Failed:$failed"; exit 1; }

# Force-sync Gitea Starscream with this checkout (branch + tags) — the nightlies diverge
sync-gitea-starscream:
    git push gitea-starscream "$(git branch --show-current)" --force
    git push gitea-starscream --tags --force

# Add the Gitea remote if it is missing
setup-remotes:
    git remote get-url origin >/dev/null 2>&1 || git remote add origin git@github.com:sorinirimies/zed-tokenburn.git
    git remote get-url gitea-starscream >/dev/null 2>&1 || git remote add gitea-starscream gitea@192.168.1.44:sorin/zed-tokenburn.git
    git remote -v

# Remove build artifacts
clean:
    cargo clean
    rm -rf dist
