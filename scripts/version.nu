#!/usr/bin/env nu
# Prints the current workspace version from Cargo.toml.
# Usage: nu scripts/version.nu

export def read_version [] {
    open Cargo.toml | get workspace.package.version
}

def main [] { print (read_version) }
