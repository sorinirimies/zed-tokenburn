//! Whatever arrives on stdin, the MCP server answers with well-formed JSON lines
//! (or nothing, for notifications) and never panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tokenburn_zed::config::Config;
use tokenburn_zed::stats::Data;

fn empty(_: chrono::DateTime<chrono::Utc>) -> Data {
    Data::default()
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut out = Vec::new();
    tokenburn_zed::mcp::serve(text.as_bytes(), &mut out, &Config::default(), &empty).unwrap();
    for line in String::from_utf8(out).unwrap().lines() {
        let v: serde_json::Value = serde_json::from_str(line).expect("a JSON reply");
        let replies = match &v {
            serde_json::Value::Array(a) => a.clone(),
            other => vec![other.clone()],
        };
        for r in replies {
            assert_eq!(r["jsonrpc"], "2.0");
            assert!(r.get("result").is_some() != r.get("error").is_some());
        }
    }
});
