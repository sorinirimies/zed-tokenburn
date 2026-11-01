# Changelog

All notable changes to this project are documented here. It is generated from
[Conventional Commits](https://www.conventionalcommits.org) by
[git-cliff](https://git-cliff.org) — do not edit it by hand.

## [Unreleased]

### ✨ Features

- Open tokenburn's desktop and web dashboards from Zed ([`9298c49`](https://github.com/sorinirimies/zed-tokenburn/commit/9298c492fdc0f7564f506fd4497a138a1c4dd885))
- Detect LSP or MCP from the first message when started without a mode ([`d934957`](https://github.com/sorinirimies/zed-tokenburn/commit/d934957543e052ac827dd321605516cf78470466))
- Tokenburn-zed language server and MCP server on tokenburn-core ([`2b13f58`](https://github.com/sorinirimies/zed-tokenburn/commit/2b13f5844b6303350d929e98a9911bec86fe6ab1))

### 🧩 Zed integration

- Move the MCP server into its own tokenburn-mcp-server extension ([`c1f47c2`](https://github.com/sorinirimies/zed-tokenburn/commit/c1f47c2375a90eda61edadf4fb9894f39433e8d1))
- Reuse the language server's binary for the context server ([`8566edb`](https://github.com/sorinirimies/zed-tokenburn/commit/8566edb2032bc26a2731fcfc84a2d4408cae572e))
- Offer the code actions on the first line by default ([`4489a77`](https://github.com/sorinirimies/zed-tokenburn/commit/4489a77d7893e5d03394b3b644018e77c7854af3))
- Zed extension that downloads and starts tokenburn-zed ([`aea4dd9`](https://github.com/sorinirimies/zed-tokenburn/commit/aea4dd93c37b2de690aa51a824ad242483efce59))

### ♻️ Refactor

- **ext:** Pure, unit-tested helpers for finding and downloading the binary ([`71ea43b`](https://github.com/sorinirimies/zed-tokenburn/commit/71ea43b09f565615e43c85e787d4c064bad1435b))

### 📚 Documentation

- Two extensions and the registry publishing guide ([`cb60ded`](https://github.com/sorinirimies/zed-tokenburn/commit/cb60dedf054f488d91fbe6f24710a23aaaf6efa3))
- Dashboards, code-action placement and context-server setup ([`d681fbc`](https://github.com/sorinirimies/zed-tokenburn/commit/d681fbc58040e16cbc5d2271766b261d20e5a05f))
- README ([`94fc7e4`](https://github.com/sorinirimies/zed-tokenburn/commit/94fc7e4cb59f76080c1624c00aba658181af99b6))

### 🧪 Tests

- Cargo-fuzz targets for settings, MCP messages, file URIs and charts ([`244a4e2`](https://github.com/sorinirimies/zed-tokenburn/commit/244a4e2e59a1cdff4b79fd3473a427daccccd1d2))

### 🔄 CI and release

- Check both extensions, build them the way Zed does ([`b9a0caa`](https://github.com/sorinirimies/zed-tokenburn/commit/b9a0caa1cb31cd281314cbe367d01c7c837c0013))
- Nushell release scripts, quality gate, GitHub and Gitea workflows ([`71836f6`](https://github.com/sorinirimies/zed-tokenburn/commit/71836f67a2179798791c6a78b7619e485d922709))


