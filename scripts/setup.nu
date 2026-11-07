#!/usr/bin/env nu
# ──────────────────────────────────────────────────────────────────────────────
# TokenBurn for Zed — Set up this machine
# ──────────────────────────────────────────────────────────────────────────────
# 1. Installs the tokenburn-zed binary into the cargo bin directory: from
#    crates.io (cargo install), or the GitHub release when cargo is missing or
#    the install fails.
# 2. Builds both extensions to extension.wasm and links them into Zed's
#    extensions directory as dev extensions, the way "zed: install dev
#    extension" does (macOS and Linux). On Windows, or with --no-link, it prints
#    the steps to do in Zed instead.
# 3. Prints the setting that points the MCP server at the binary.
#
# Usage:
#   nu scripts/setup.nu                     crates.io first, then GitHub
#   nu scripts/setup.nu --from github       the GitHub release only
#   nu scripts/setup.nu --version 0.1.1     a given version instead of the latest
#   nu scripts/setup.nu --no-link           binary only; install the extensions in Zed
# ──────────────────────────────────────────────────────────────────────────────

use ci/package.nu [asset_name binary_name]
use ci/zed_build.nu [EXTENSIONS wasm_path]

const root = path self ..

export const REPO = "sorinirimies/zed-tokenburn"
export const CRATE = "tokenburn-zed"

# Release target per `os/arch` (mirrors src/platform.rs `target`). Linux builds
# are static; Windows on ARM runs the x86-64 build.
export const TARGETS = {
    "macos/aarch64": "aarch64-apple-darwin"
    "macos/x86_64": "x86_64-apple-darwin"
    "linux/aarch64": "aarch64-unknown-linux-musl"
    "linux/x86_64": "x86_64-unknown-linux-musl"
    "windows/x86_64": "x86_64-pc-windows-msvc"
    "windows/aarch64": "x86_64-pc-windows-msvc"
}

# The release target for an OS and CPU, as `$nu.os-info` names them.
export def release_target [os: string, arch: string]: nothing -> any {
    let target = $TARGETS | get -o $"($os)/($arch)"
    if $target == null {
        error make {msg: $"no ($CRATE) release for ($os) ($arch); install it with `cargo install ($CRATE)`"}
    }
    $target
}

# Where to try the binary from, in order. `from` is `crates` or `github`.
export def sources [from: string, has_cargo: bool]: nothing -> any {
    match $from {
        "crates" => (if $has_cargo { ["crates" "github"] } else { ["github"] })
        "github" => ["github"]
        _ => (error make {msg: $"--from must be crates or github, not ($from)"})
    }
}

# Download URL of the release archive; an empty version means the latest release.
export def release_url [target: string, version: string]: nothing -> string {
    let asset = asset_name $target
    let v = $version | str replace -r '^v' ''
    if ($v | is-empty) {
        $"https://github.com/($REPO)/releases/latest/download/($asset)"
    } else {
        $"https://github.com/($REPO)/releases/download/v($v)/($asset)"
    }
}

# The checksum in a `.sha256` file (`<sum>  <file>`).
export def checksum_of [text: string]: nothing -> string {
    $text | str trim | split row -r '\s+' | first | str lowercase
}

# Zed's extensions directory.
export def zed_extensions_dir [os: string, home: string, env_vars: record]: nothing -> string {
    match $os {
        "macos" => ($home | path join "Library" "Application Support" "Zed" "extensions")
        "windows" => ($env_vars | get -o LOCALAPPDATA | default "" | path join "Zed" "extensions")
        _ => {
            let xdg = $env_vars | get -o XDG_DATA_HOME | default ""
            let data = if ($xdg | is-empty) { $home | path join ".local" "share" } else { $xdg }
            $data | path join "zed" "extensions"
        }
    }
}

# The directory `cargo install` puts binaries in.
export def cargo_bin_dir [home: string, env_vars: record]: nothing -> string {
    let cargo_home = $env_vars | get -o CARGO_HOME | default ""
    let base = if ($cargo_home | is-empty) { $home | path join ".cargo" } else { $cargo_home }
    $base | path join "bin"
}

# What to do with a link in Zed's `installed/` directory.
#   kind:    what is there now (`path type`: null, symlink, dir, file)
#   current: where it resolves to (`path expand`), wanted: this checkout
# Returns create, keep, replace (a link to another place) or blocked (a real
# directory or file: an extension installed from the registry, left alone).
export def link_action [kind: any, current: string, wanted: string]: nothing -> string {
    match $kind {
        null => "create"
        "symlink" => (if $current == $wanted { "keep" } else { "replace" })
        _ => "blocked"
    }
}

# The dev extensions of this checkout: id, source directory, link in Zed and the
# wasm crate.
export def extension_links [repo: string, ext_dir: string]: nothing -> table {
    $EXTENSIONS | each {|e|
        let dir = $repo | path join $e.dir | path expand
        let id = open ($dir | path join "extension.toml") | get id
        {id: $id, dir: $dir, link: ($ext_dir | path join "installed" $id), crate: $e.crate}
    }
}

# The settings that make the MCP server use the installed binary.
export def mcp_settings [bin: string]: nothing -> string {
    {context_servers: {tokenburn: {settings: {binary_path: $bin}}}} | to json
}

def has_cmd [name: string]: nothing -> bool {
    which $name | where type == external | is-not-empty
}

def from_crates [version: string]: nothing -> bool {
    let v = $version | str replace -r '^v' ''
    let args = if ($v | is-empty) { [] } else { ["--version" $v] }
    try { ^cargo install $CRATE --locked ...$args; true } catch { false }
}

def from_github [target: string, version: string, bin_dir: string]: nothing -> nothing {
    let url = release_url $target $version
    let tmp = mktemp -d
    let archive = $tmp | path join (asset_name $target)
    print -e $"Downloading ($url)"
    http get --raw $url | save -f $archive
    let want = checksum_of (http get --raw $"($url).sha256" | decode utf-8)
    let got = open --raw $archive | hash sha256
    if $got != $want {
        rm -rf $tmp
        error make {msg: $"checksum mismatch for ($url): expected ($want), got ($got)"}
    }
    ^tar -xf $archive -C $tmp
    mkdir $bin_dir
    let bin = binary_name $target
    cp -f ($tmp | path join $bin) ($bin_dir | path join $bin)
    if $nu.os-info.name != "windows" { ^chmod +x ($bin_dir | path join $bin) }
    rm -rf $tmp
}

def build_wasm [e: record]: nothing -> nothing {
    let target_dir = $e.dir | path join "target"
    do { cd $e.dir; ^cargo build --target wasm32-wasip2 --release --target-dir $target_dir }
    cp -f (wasm_path $e.dir $e.crate) ($e.dir | path join "extension.wasm")
}

def link [e: record]: nothing -> string {
    let kind = $e.link | path type
    let current = if $kind == "symlink" { $e.link | path expand } else { "" }
    let action = link_action $kind $current $e.dir
    match $action {
        "create" | "replace" => {
            mkdir ($e.link | path dirname)
            ^ln -sfn $e.dir $e.link
        }
        "blocked" => {
            print -e $"(ansi yellow)⚠ ($e.link) is an installed extension; uninstall ($e.id) in Zed, then run this again(ansi reset)"
        }
        _ => {}
    }
    $action
}

def main [
    --from: string = "crates"   # crates (GitHub as fallback) or github
    --version: string = ""      # the version to install (default: the latest)
    --no-link                   # don't link the extensions into Zed; print the steps instead
] {
    let os = $nu.os-info.name
    let home = $nu.home-dir
    let target = release_target $os $nu.os-info.arch
    let bin_dir = cargo_bin_dir $home $env
    let bin = $bin_dir | path join (binary_name $target)

    # 1. The binary.
    mut installed = false
    for source in (sources $from (has_cmd cargo)) {
        if $installed { break }
        print -e $"(ansi cyan)── ($CRATE) from ($source)(ansi reset)"
        if $source == "crates" {
            $installed = from_crates $version
            if not $installed { print -e $"(ansi yellow)⚠ cargo install failed; trying the GitHub release(ansi reset)" }
        } else {
            from_github $target $version $bin_dir
            $installed = true
        }
    }
    print $"binary=($bin)"

    # 2. The extensions.
    let ext_dir = zed_extensions_dir $os $home $env
    let links = extension_links $root $ext_dir
    let can_link = (not $no_link) and $os != "windows" and (has_cmd cargo)
    if $can_link {
        if (has_cmd rustup) { ^rustup target add wasm32-wasip2 }
        let actions = $links | each {|e|
            print -e $"(ansi cyan)── extension ($e.id)(ansi reset)"
            build_wasm $e
            {id: $e.id, action: (link $e)}
        }
        for a in $actions { print $"extension.($a.id)=($a.action)" }
        print -e $"(ansi green)✅ Restart Zed to load the extensions.(ansi reset)"
    } else {
        print -e "In Zed: command palette → 'zed: install dev extension', pick each folder:"
        for e in $links { print -e $"  ($e.dir)" }
    }

    # 3. The MCP server setting.
    print -e "\nFor the agent tools, add this to Zed's settings.json:"
    print -e (mcp_settings $bin)
    print -e "Open a project and trust it when Zed asks; the status bar shows 'TokenBurn: 🔥 …'."
}
