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

# ── Changelog ─────────────────────────────────────────────────────────────────

# Regenerate CHANGELOG.md from all tags
changelog: _check-git-cliff
    git-cliff --output CHANGELOG.md

# Preview the next release's changelog
changelog-preview: _check-git-cliff
    @git-cliff --unreleased

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

# Bump Cargo.toml + extension.toml + Cargo.lock, regenerate the changelog, commit and tag
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

# Add the Gitea remote if it is missing
setup-remotes:
    git remote get-url origin >/dev/null 2>&1 || git remote add origin git@github.com:sorinirimies/zed-tokenburn.git
    git remote get-url gitea-starscream >/dev/null 2>&1 || git remote add gitea-starscream gitea@192.168.1.44:sorin/zed-tokenburn.git
    git remote -v

# Remove build artifacts
clean:
    cargo clean
    rm -rf dist
