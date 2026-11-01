//! Number formatting shared by the status bar, the report and the MCP tools.

/// `1_234_567` → `1.23M`, `52_400` → `52.4k`, `999` → `999` (pi-tokenburn's format).
pub fn tokens(n: u64) -> String {
    let f = n as f64;
    if n >= 1_000_000_000 {
        format!("{:.2}B", f / 1e9)
    } else if n >= 1_000_000 {
        format!("{:.2}M", f / 1e6)
    } else if n >= 1_000 {
        format!("{:.1}k", f / 1e3)
    } else {
        n.to_string()
    }
}

/// A fraction as a whole percent: `0.953` → `95%`.
pub fn percent(fraction: f64) -> String {
    format!("{:.0}%", fraction * 100.0)
}

/// Share of `part` in `total`, `–` when the total is zero.
pub fn share(part: u64, total: u64) -> String {
    if total == 0 {
        "–".into()
    } else {
        percent(part as f64 / total as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_use_short_units() {
        assert_eq!(tokens(0), "0");
        assert_eq!(tokens(999), "999");
        assert_eq!(tokens(1_000), "1.0k");
        assert_eq!(tokens(52_400), "52.4k");
        assert_eq!(tokens(1_234_567), "1.23M");
        assert_eq!(tokens(7_000_000_000), "7.00B");
    }

    #[test]
    fn percent_and_share_round() {
        assert_eq!(percent(0.953), "95%");
        assert_eq!(share(1, 3), "33%");
        assert_eq!(share(5, 0), "–");
    }
}
