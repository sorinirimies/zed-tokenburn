# Changelog

All notable changes to this project are documented here. It is generated from
[Conventional Commits](https://www.conventionalcommits.org) by
[git-cliff](https://git-cliff.org) — do not edit it by hand.

## [0.1.1](https://github.com/sorinirimies/zed-tokenburn/compare/v0.1.0...v0.1.1) — 2026-11-02

### ✨ Features

- Move to tokenburn-core 0.3.13 and leave pi plugins' savings to pi ([`461a5d7`](https://github.com/sorinirimies/zed-tokenburn/commit/461a5d70b84e4f302905fe3936c11ebaaeab90b3))

### 🐛 Bug fixes

- **scripts:** Show git's error when a remote operation fails ([`2bddaa3`](https://github.com/sorinirimies/zed-tokenburn/commit/2bddaa32fa1a348bfdb22bc4ddf83a81bbfb1277))

### 📚 Documentation

- Drop the per-repo agent rules skill; the shared skills cover it ([`7bfe1d0`](https://github.com/sorinirimies/zed-tokenburn/commit/7bfe1d0a5cd0e1d6164e8a8ecfe9065ad0dee848))
- This repo's agent rules as a project skill in .agents/skills ([`5153bc7`](https://github.com/sorinirimies/zed-tokenburn/commit/5153bc793fa5ac91d49b369cb0a0aab35d57f963))
- Add demo recordings to the README ([`7191d74`](https://github.com/sorinirimies/zed-tokenburn/commit/7191d747975423d3071a43238839712248faf2b6))

### 📦 Dependencies

- Nightly cargo update 2026-11-02 ([`857d15a`](https://github.com/sorinirimies/zed-tokenburn/commit/857d15a1d320377afdd78e6875c34eea19eef469))
- Bump dirs from 6.0.0 to 7.0.0 (#4) ([`212759e`](https://github.com/sorinirimies/zed-tokenburn/commit/212759e4a96e0c696467dbb7efbcfe5eb4485788))

### 🔄 CI and release

- Let Dependabot scan only the root manifest ([`29c1654`](https://github.com/sorinirimies/zed-tokenburn/commit/29c16547d20c2044be406a018cbacb86983ab729))
- Split CI into one job per quality-gate step on both hosts and name every step ([`8946d16`](https://github.com/sorinirimies/zed-tokenburn/commit/8946d16afa514fba9644b28a8ea4b827fdf26c62))
- Merge green Dependabot PRs after CI and let Dependabot bump cargo dependencies ([`84d0647`](https://github.com/sorinirimies/zed-tokenburn/commit/84d06479da8290837ffe3a20b962a9ef56ea7828))
- Bump softprops/action-gh-release from 2 to 3 (#2) ([`b3bc42a`](https://github.com/sorinirimies/zed-tokenburn/commit/b3bc42a88487ec1882d1371b2ad26d1c9782407f))
- Bump dependabot/fetch-metadata from 2 to 3 (#1) ([`2621721`](https://github.com/sorinirimies/zed-tokenburn/commit/2621721f126ea3343b21c448ffae0a40875bd0dc))

### 🔧 Chores

- Ignore .codegraph index ([`3570e70`](https://github.com/sorinirimies/zed-tokenburn/commit/3570e70374df49d9b5b0a5d6eb8b21f3b9ef4237))

## [0.1.0](https://github.com/sorinirimies/zed-tokenburn/releases/tag/v0.1.0) — 2026-11-01

### ✨ Features

- Open tokenburn's desktop and web dashboards from Zed ([`fb3f3e5`](https://github.com/sorinirimies/zed-tokenburn/commit/fb3f3e5a4b9e6548b2d6bd31d76e3e8b8d7b27d8))
- Detect LSP or MCP from the first message when started without a mode ([`a298cb3`](https://github.com/sorinirimies/zed-tokenburn/commit/a298cb3d49823aedf1e234c201c3d72661a92514))
- Tokenburn-zed language server and MCP server on tokenburn-core ([`2b13f58`](https://github.com/sorinirimies/zed-tokenburn/commit/2b13f5844b6303350d929e98a9911bec86fe6ab1))

### 🧩 Zed integration

- Move the MCP server into its own tokenburn-mcp-server extension ([`2e4aff5`](https://github.com/sorinirimies/zed-tokenburn/commit/2e4aff5a8afffdd482bda4ff8328b5e7093715e0))
- Reuse the language server's binary for the context server ([`cefdb34`](https://github.com/sorinirimies/zed-tokenburn/commit/cefdb3487fdc0181dd43e857e2f3caded26d7382))
- Offer the code actions on the first line by default ([`24a0dd6`](https://github.com/sorinirimies/zed-tokenburn/commit/24a0dd6e6ae50b6a6e0a19a942e9a3cb257a5c1c))
- Zed extension that downloads and starts tokenburn-zed ([`aea4dd9`](https://github.com/sorinirimies/zed-tokenburn/commit/aea4dd93c37b2de690aa51a824ad242483efce59))

### ♻️ Refactor

- **ext:** Pure, unit-tested helpers for finding and downloading the binary ([`309ccf1`](https://github.com/sorinirimies/zed-tokenburn/commit/309ccf1b1dcf355bbbbba5eb79432b36b0e506fa))

### 📚 Documentation

- Two extensions and the registry publishing guide ([`b615b06`](https://github.com/sorinirimies/zed-tokenburn/commit/b615b06f219561e00c7056e57f531e8f10cfee60))
- Dashboards, code-action placement and context-server setup ([`0e21fa6`](https://github.com/sorinirimies/zed-tokenburn/commit/0e21fa6736f3955ace778edc0aacafe5ebc5d53f))
- README ([`8f63b01`](https://github.com/sorinirimies/zed-tokenburn/commit/8f63b01aadd3e59c190175b8435729caa9eec317))

### 🧪 Tests

- Expect the platform separator in absolute_joins_the_extension_dir ([`f90d1c6`](https://github.com/sorinirimies/zed-tokenburn/commit/f90d1c6ae0724b4d4940ee90e03702b3ca0acd43))
- Cargo-fuzz targets for settings, MCP messages, file URIs and charts ([`244a4e2`](https://github.com/sorinirimies/zed-tokenburn/commit/244a4e2e59a1cdff4b79fd3473a427daccccd1d2))

### 📦 Build

- Nushell-only tooling from the create-rust-project template ([`855a77b`](https://github.com/sorinirimies/zed-tokenburn/commit/855a77b971d8b6a3a8813bd7cf909288908098e5))
- **just:** Remote, release, dependency and nightly-on-demand recipes ([`1494db2`](https://github.com/sorinirimies/zed-tokenburn/commit/1494db28164fcaf10f8dfbfe3a9ab17ee6239017))

### 🔄 CI and release

- **fuzz:** Fall back to an older nightly when the latest cannot build ([`6828870`](https://github.com/sorinirimies/zed-tokenburn/commit/682887045d1f83872f7bc591f51107e7795c8ffe))
- Pin the MSRV toolchain via @master so Dependabot leaves it alone ([`617d372`](https://github.com/sorinirimies/zed-tokenburn/commit/617d37266448eae21af28dd541e66aa8346563ed))
- Nightly dependency update on Gitea, keep both hosts in sync ([`1039e87`](https://github.com/sorinirimies/zed-tokenburn/commit/1039e872410d8dbffb80c1e1aa506523a426871a))
- Check both extensions, build them the way Zed does ([`cb0ec61`](https://github.com/sorinirimies/zed-tokenburn/commit/cb0ec614890b4216762b1ff80fbd2930201d3bd3))
- Nushell release scripts, quality gate, GitHub and Gitea workflows ([`71836f6`](https://github.com/sorinirimies/zed-tokenburn/commit/71836f67a2179798791c6a78b7619e485d922709))


