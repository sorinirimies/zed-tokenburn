//! Numbers for one span: totals, burn rate, cache hit ratio and savings.
//!
//! Everything here is pure: rows in, numbers out. Reading the logs is
//! [`load`]'s job (one `tokenburn_core::collect` call for every tool).

use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use tokenburn_core::{ExternalSavings, Row, Savings, Summary, Tool, ToolFilter};

use crate::span::Span;

/// Rows of every tool since some start, plus the savings that live outside them.
#[derive(Debug, Clone, Default)]
pub struct Data {
    /// Sorted by time.
    pub rows: Vec<Row>,
    pub external: ExternalSavings,
    /// Non-fatal read problems, from `tokenburn-core`.
    pub warnings: Vec<String>,
}

/// Read every tool's logs from `start` on. Spending limits always count every
/// tool, so the tool filter is applied afterwards, in memory.
pub fn load(start: DateTime<Utc>) -> Data {
    let c = tokenburn_core::collect(ToolFilter::All, start);
    Data {
        rows: c.rows,
        external: c.external,
        warnings: c.warnings,
    }
}

/// Tokens of a summary, with or without the cache read/write tokens.
pub fn tokens_of(s: &Summary, include_cache: bool) -> u64 {
    if include_cache {
        s.total_tokens
    } else {
        s.input + s.output
    }
}

/// Tokens of one row, with or without the cache tokens.
pub fn row_tokens(r: &Row, include_cache: bool) -> u64 {
    if include_cache {
        r.input + r.output + r.cache_read + r.cache_write
    } else {
        r.input + r.output
    }
}

/// Share of the prompt that came from the cache: `cache_read / (input + cache_read +
/// cache_write)`. `None` when there was no prompt at all.
pub fn hit_ratio(s: &Summary) -> Option<f64> {
    let prompt = s.input + s.cache_read + s.cache_write;
    (prompt > 0).then(|| s.cache_read as f64 / prompt as f64)
}

/// The external savings that belong to `tool`: rtk cannot tell which agent ran a
/// command (so it counts only for "all"), compactions are pi's.
pub fn external_for(tool: ToolFilter, ext: &ExternalSavings) -> ExternalSavings {
    match tool {
        ToolFilter::All => ext.clone(),
        ToolFilter::Only(Tool::Pi) => ExternalSavings {
            compaction_tokens: ext.compaction_tokens,
            compactions: ext.compactions,
            compaction_events: ext.compaction_events.clone(),
            ..Default::default()
        },
        ToolFilter::Only(_) => ExternalSavings::default(),
    }
}

/// Everything shown for one span.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpanStats {
    pub span: Span,
    pub summary: Summary,
    /// Tokens as configured (with or without cache tokens).
    pub tokens: u64,
    pub cost_usd: f64,
    /// Tokens per hour, averaged over the span (from its first turn when the span
    /// started earlier; at least one hour).
    pub tokens_per_hour: f64,
    pub cost_per_hour: f64,
    pub hit_ratio: Option<f64>,
    pub savings: Savings,
}

/// Totals for `span` over `rows`, keeping only `tool`'s.
pub fn span_stats(
    data: &Data,
    tool: ToolFilter,
    span: Span,
    now: DateTime<Local>,
    include_cache: bool,
    input_price: f64,
) -> SpanStats {
    let start = span.start(now);
    let rows: Vec<&Row> = data
        .rows
        .iter()
        .filter(|r| r.ts >= start && tool.includes(r.tool))
        .collect();
    let summary = Summary::of(rows.iter().copied());
    let tokens = tokens_of(&summary, include_cache);
    let first = rows.iter().map(|r| r.ts).min();
    let from = first.map_or(start, |f| f.max(start));
    let hours = ((now.with_timezone(&Utc) - from).num_seconds() as f64 / 3600.0).max(1.0);
    let ext = external_for(tool, &data.external).within(|ts| ts >= start);
    SpanStats {
        span,
        summary,
        tokens,
        cost_usd: summary.cost_usd,
        tokens_per_hour: tokens as f64 / hours,
        cost_per_hour: summary.cost_usd / hours,
        hit_ratio: hit_ratio(&summary),
        savings: Savings::build(rows.iter().copied(), &ext, input_price),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    pub(crate) fn row(
        tool: Tool,
        project: &str,
        ts: DateTime<Local>,
        tokens: u64,
        cost: f64,
    ) -> Row {
        Row {
            tool,
            project: project.into(),
            id: "id".into(),
            ts: ts.with_timezone(&Utc),
            input: tokens,
            output: tokens / 2,
            cache_read: tokens * 2,
            cache_write: 0,
            cost,
        }
    }

    pub(crate) fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 5, 6, 15, 0, 0).unwrap()
    }

    fn data(rows: Vec<Row>) -> Data {
        Data {
            rows,
            ..Default::default()
        }
    }

    #[test]
    fn totals_follow_the_span_and_the_tool() {
        let n = now();
        let d = data(vec![
            row(Tool::Pi, "a", n - Duration::days(40), 1000, 1.0),
            row(Tool::Pi, "a", n - Duration::hours(2), 100, 0.5),
            row(Tool::Claude, "b", n - Duration::minutes(10), 10, 0.25),
        ]);
        let today = span_stats(&d, ToolFilter::All, Span::Today, n, true, 3.0);
        assert_eq!(today.summary.turns, 2);
        assert_eq!(today.tokens, 100 + 50 + 200 + 10 + 5 + 20);
        assert!((today.cost_usd - 0.75).abs() < 1e-9);
        let all_pi = span_stats(&d, ToolFilter::PI, Span::All, n, false, 3.0);
        assert_eq!(all_pi.summary.turns, 2);
        assert_eq!(all_pi.tokens, 1000 + 500 + 100 + 50, "without cache tokens");
        let hour = span_stats(&d, ToolFilter::All, Span::Hour, n, true, 3.0);
        assert_eq!(hour.summary.turns, 1);
    }

    #[test]
    fn empty_input_gives_zeros_not_nan() {
        let s = span_stats(
            &Data::default(),
            ToolFilter::All,
            Span::Week,
            now(),
            true,
            3.0,
        );
        assert_eq!(s.tokens, 0);
        assert_eq!(s.tokens_per_hour, 0.0);
        assert_eq!(s.hit_ratio, None);
        assert!(s.savings.is_empty());
    }

    #[test]
    fn rate_counts_from_the_first_turn_and_at_least_one_hour() {
        let n = now();
        // One turn 4 h ago today: 4 h of activity, not 15 h since midnight.
        let d = data(vec![row(Tool::Pi, "a", n - Duration::hours(4), 400, 4.0)]);
        let s = span_stats(&d, ToolFilter::All, Span::Today, n, false, 3.0);
        assert!(
            (s.tokens_per_hour - 150.0).abs() < 1e-9,
            "{}",
            s.tokens_per_hour
        );
        assert!((s.cost_per_hour - 1.0).abs() < 1e-9);
        // A turn a minute ago: divided by one hour, not by one minute.
        let d = data(vec![row(Tool::Pi, "a", n - Duration::minutes(1), 400, 0.0)]);
        let s = span_stats(&d, ToolFilter::All, Span::Today, n, false, 3.0);
        assert!((s.tokens_per_hour - 600.0).abs() < 1e-9);
    }

    #[test]
    fn hit_ratio_is_cache_reads_over_the_whole_prompt() {
        let s = Summary {
            input: 10,
            cache_read: 80,
            cache_write: 10,
            ..Default::default()
        };
        assert_eq!(hit_ratio(&s), Some(0.8));
        assert_eq!(hit_ratio(&Summary::default()), None);
    }

    #[test]
    fn savings_count_the_cache_and_the_external_sources_per_tool() {
        let ext = ExternalSavings {
            rtk_tokens: 1_000_000,
            rtk_commands: 3,
            compaction_tokens: 500_000,
            compactions: 1,
            ..Default::default()
        };
        assert_eq!(external_for(ToolFilter::All, &ext), ext);
        let pi = external_for(ToolFilter::PI, &ext);
        assert_eq!((pi.rtk_tokens, pi.compaction_tokens), (0, 500_000));
        let zed = external_for(ToolFilter::Only(Tool::Zed), &ext);
        assert_eq!(zed, ExternalSavings::default());
    }
}
