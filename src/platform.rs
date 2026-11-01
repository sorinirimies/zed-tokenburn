//! Which release asset fits this machine. Pure, so it is unit-tested natively.

/// Operating system (mirrors `zed::Os`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Linux,
    Windows,
}

/// CPU architecture (mirrors `zed::Architecture`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arch {
    Aarch64,
    X86,
    X86_64,
}

/// Rust target triple of the release binary for `os`/`arch`.
///
/// Linux binaries are static (musl), so they run on any distribution. Windows on
/// ARM runs the x86-64 build under emulation.
pub fn target(os: Os, arch: Arch) -> Result<&'static str, String> {
    match (os, arch) {
        (Os::Mac, Arch::Aarch64) => Ok("aarch64-apple-darwin"),
        (Os::Mac, Arch::X86_64) => Ok("x86_64-apple-darwin"),
        (Os::Linux, Arch::Aarch64) => Ok("aarch64-unknown-linux-musl"),
        (Os::Linux, Arch::X86_64) => Ok("x86_64-unknown-linux-musl"),
        (Os::Windows, Arch::X86_64 | Arch::Aarch64) => Ok("x86_64-pc-windows-msvc"),
        (os, arch) => Err(format!(
            "no tokenburn-zed release for {os:?} {arch:?}; build it with \
             `cargo install tokenburn-zed` and put it on PATH"
        )),
    }
}

pub fn is_windows(target: &str) -> bool {
    target.contains("windows")
}

/// `tokenburn-zed-aarch64-apple-darwin.tar.gz` / `….zip` on Windows.
pub fn asset_name(target: &str) -> String {
    let ext = if is_windows(target) { "zip" } else { "tar.gz" };
    format!("tokenburn-zed-{target}.{ext}")
}

/// The binary inside the unpacked archive (`dir/tokenburn-zed[.exe]`).
pub fn binary_path(dir: &str, target: &str) -> String {
    let exe = if is_windows(target) { ".exe" } else { "" };
    format!("{dir}/tokenburn-zed{exe}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_supported_platform_has_a_target() {
        assert_eq!(
            target(Os::Mac, Arch::Aarch64).unwrap(),
            "aarch64-apple-darwin"
        );
        assert_eq!(
            target(Os::Mac, Arch::X86_64).unwrap(),
            "x86_64-apple-darwin"
        );
        assert_eq!(
            target(Os::Linux, Arch::X86_64).unwrap(),
            "x86_64-unknown-linux-musl"
        );
        assert_eq!(
            target(Os::Linux, Arch::Aarch64).unwrap(),
            "aarch64-unknown-linux-musl"
        );
        assert_eq!(
            target(Os::Windows, Arch::X86_64).unwrap(),
            "x86_64-pc-windows-msvc"
        );
        assert_eq!(
            target(Os::Windows, Arch::Aarch64).unwrap(),
            "x86_64-pc-windows-msvc",
            "Windows on ARM emulates x86-64"
        );
    }

    #[test]
    fn thirty_two_bit_is_unsupported() {
        for os in [Os::Mac, Os::Linux, Os::Windows] {
            let e = target(os, Arch::X86).unwrap_err();
            assert!(e.contains("cargo install tokenburn-zed"), "{e}");
        }
    }

    #[test]
    fn asset_and_binary_names_match_the_release_workflow() {
        assert_eq!(
            asset_name("aarch64-apple-darwin"),
            "tokenburn-zed-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            asset_name("x86_64-pc-windows-msvc"),
            "tokenburn-zed-x86_64-pc-windows-msvc.zip"
        );
        assert_eq!(
            binary_path("d", "x86_64-unknown-linux-musl"),
            "d/tokenburn-zed"
        );
        assert_eq!(
            binary_path("d", "x86_64-pc-windows-msvc"),
            "d/tokenburn-zed.exe"
        );
    }
}
