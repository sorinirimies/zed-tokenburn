//! The status-bar text.
//!
//! Zed has no extension API for status-bar items, but it shows language-server
//! progress there. The language server keeps one progress open for its whole
//! life, titled `TokenBurn`, and puts this text in its message — so the bottom of
//! the window reads
//!
//! ```text
//! TokenBurn: 🔥 1.20M ($5.10) [today] · 52.4k/h · 95% hit · ~$12.40 saved
//! ```
//!
//! the same line pi-tokenburn shows in pi's footer.

use tokenburn_core::{Currency, Level, ToolFilter};

use crate::config::Config;
use crate::fmt;
use crate::stats::SpanStats;

/// Savings below this many dollars are not worth a mention.
const MIN_SAVED_USD: f64 = 0.005;

/// The text after `TokenBurn: `.
pub fn line(stats: &SpanStats, cfg: &Config, level: Level, cur: &Currency) -> String {
    let mut head = match level {
        Level::Over => "🔥 OVER BUDGET ".to_string(),
        Level::Warn => "🔥 ⚠ ".to_string(),
        Level::Ok => "🔥 ".to_string(),
    };
    head.push_str(&fmt::tokens(stats.tokens));
    if cfg.show_cost {
        head.push_str(&format!(" ({})", cur.format(stats.cost_usd)));
    }
    let scope = match cfg.tool {
        ToolFilter::All => stats.span.as_str().to_string(),
        ToolFilter::Only(t) => format!("{} · {}", stats.span.as_str(), t.as_str()),
    };
    head.push_str(&format!(" [{scope}]"));

    let mut extras = Vec::new();
    if cfg.show_rate && stats.tokens > 0 {
        extras.push(format!(
            "{}/h",
            fmt::tokens(stats.tokens_per_hour.round() as u64)
        ));
    }
    if cfg.show_hit {
        if let Some(hit) = stats.hit_ratio {
            extras.push(format!("{} hit", fmt::percent(hit)));
        }
    }
    if cfg.show_saved && stats.savings.usd >= MIN_SAVED_USD {
        extras.push(format!("~{} saved", cur.format(stats.savings.usd)));
    }
    if extras.is_empty() {
        head
    } else {
        format!("{head} · {}", extras.join(" · "))
    }
}

/// Shown while paused (`enabled: false` or the "pause" code action).
pub const PAUSED: &str = "⏸ paused";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;
    use serde_json::json;
    use tokenburn_core::{Savings, Summary};

    fn stats() -> SpanStats {
        SpanStats {
            span: Span::Today,
            summary: Summary::default(),
            tokens: 1_200_000,
            cost_usd: 5.1,
            tokens_per_hour: 52_400.0,
            cost_per_hour: 0.4,
            hit_ratio: Some(0.953),
            savings: Savings {
                usd: 12.4,
                tokens: 4_000_000,
                sources: Vec::new(),
            },
        }
    }

    #[test]
    fn full_line_matches_pi_tokenburn() {
        let s = line(&stats(), &Config::default(), Level::Ok, &Currency::USD);
        assert_eq!(
            s,
            "🔥 1.20M ($5.10) [today] · 52.4k/h · 95% hit · ~$12.40 saved"
        );
    }

    #[test]
    fn extras_can_be_switched_off() {
        let cfg = Config::from_layers([&json!({
            "show_rate": false, "show_hit": false, "show_saved": false, "show_cost": false
        })]);
        assert_eq!(
            line(&stats(), &cfg, Level::Ok, &Currency::USD),
            "🔥 1.20M [today]"
        );
    }

    #[test]
    fn budget_levels_and_tool_filter_show_up() {
        let cfg = Config::from_layers([&json!({ "tool": "pi", "show_rate": false,
            "show_hit": false, "show_saved": false })]);
        assert_eq!(
            line(&stats(), &cfg, Level::Over, &Currency::USD),
            "🔥 OVER BUDGET 1.20M ($5.10) [today · pi]"
        );
        assert!(line(&stats(), &cfg, Level::Warn, &Currency::USD).starts_with("🔥 ⚠ 1.20M"));
    }

    #[test]
    fn nothing_used_hides_the_rate_hit_and_savings() {
        let empty = SpanStats {
            tokens: 0,
            cost_usd: 0.0,
            tokens_per_hour: 0.0,
            hit_ratio: None,
            savings: Savings::default(),
            ..stats()
        };
        assert_eq!(
            line(&empty, &Config::default(), Level::Ok, &Currency::USD),
            "🔥 0 ($0.00) [today]"
        );
    }

    #[test]
    fn cost_and_savings_use_the_display_currency() {
        let eur = tokenburn_core::currency_by_code("EUR").unwrap();
        let s = line(&stats(), &Config::default(), Level::Ok, &eur);
        assert!(s.contains('€') && !s.contains('$'), "{s}");
    }
}
