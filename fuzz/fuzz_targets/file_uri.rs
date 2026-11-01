//! `file_uri` always yields an ASCII URI that percent-decodes back to the path
//! (with `\` turned into `/`).
#![no_main]

use std::path::Path;

use libfuzzer_sys::fuzz_target;
use tokenburn_zed::lsp::file_uri;

fn decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).unwrap();
            out.push(u8::from_str_radix(hex, 16).unwrap());
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

fuzz_target!(|path: &str| {
    let uri = file_uri(Path::new(path));
    assert!(uri.starts_with("file:///"), "{uri}");
    assert!(uri.bytes().all(|b| b.is_ascii_graphic()), "{uri}");
    let body = decode(&uri["file://".len()..]);
    let want = path.replace('\\', "/");
    let want = if want.starts_with('/') { want } else { format!("/{want}") };
    assert_eq!(body, want.as_bytes());
});
