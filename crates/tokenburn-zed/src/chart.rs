//! Text column charts (pi-tokenburn style), for the report and the MCP tools.
//!
//! ```text
//! Daily tokens · last 14 days  ┄ budget 150.0k
//!  291.9k │                                 ██ ▄▄
//!         │┄┄┄┄┄┄██┄┄┄┄┄┄┄┄┄┄██┄┄┄┄┄┄┄▆▆┄┄┄┄██┄██┄▅▅┄
//!  146.0k │      ██          ██       ██    ██ ██ ██
//!         └──────────────────────────────────────────
//!          26 27 28 29 30 01 02 03 04 05 06 07 08 09
//!          Sa Su Mo Tu We Th Fr Sa Su Mo Tu We Th Fr
//!                                                 ▲
//! ```

use std::str::FromStr;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokenburn_core::ToolFilter;

use crate::span::{local_midnight, week_start};
use crate::stats::{row_tokens, Data};

/// Column width of one bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum View {
    /// The last 14 days.
    #[default]
    Day,
    /// The last 8 weeks (Monday start).
    Week,
    /// The last 6 months.
    Month,
}

impl View {
    pub const ALL: [View; 3] = [View::Day, View::Week, View::Month];

    pub fn as_str(self) -> &'static str {
        match self {
            View::Day => "day",
            View::Week => "week",
            View::Month => "month",
        }
    }

    /// How many columns.
    pub fn count(self) -> usize {
        match self {
            View::Day => 14,
            View::Week => 8,
            View::Month => 6,
        }
    }

    /// `Daily` / `Weekly` / `Monthly`.
    pub fn adjective(self) -> &'static str {
        match self {
            View::Day => "Daily",
            View::Week => "Weekly",
            View::Month => "Monthly",
        }
    }

    /// `day` / `week` / `month`, for "avg … per day".
    pub fn unit(self) -> &'static str {
        self.as_str()
    }

    /// `last 14 days`.
    pub fn range(self) -> String {
        format!("last {} {}s", self.count(), self.unit())
    }
}

impl FromStr for View {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "day" | "daily" | "days" => Ok(View::Day),
            "week" | "weekly" | "weeks" => Ok(View::Week),
            "month" | "monthly" | "months" => Ok(View::Month),
            other => Err(format!("view must be day, week or month (got '{other}')")),
        }
    }
}

/// What the columns measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Metric {
    #[default]
    Tokens,
    Cost,
}

impl FromStr for Metric {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tokens" | "token" => Ok(Metric::Tokens),
            "cost" | "usd" | "money" => Ok(Metric::Cost),
            other => Err(format!("metric must be tokens or cost (got '{other}')")),
        }
    }
}

/// One bucket.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// First day of the bucket (local).
    pub start: NaiveDate,
    pub label: String,
    /// Second label line (weekday for days).
    pub sub: Option<String>,
    pub tokens: u64,
    pub cost_usd: f64,
}

impl Column {
    pub fn value(&self, metric: Metric) -> f64 {
        match metric {
            Metric::Tokens => self.tokens as f64,
            Metric::Cost => self.cost_usd,
        }
    }
}

fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap_or(d)
}

/// The bucket start dates, oldest first; the last one contains `today`.
pub fn bucket_starts(view: View, today: NaiveDate) -> Vec<NaiveDate> {
    let n = view.count() as i64;
    match view {
        View::Day => (0..n).rev().map(|i| today - Duration::days(i)).collect(),
        View::Week => {
            let this = week_start(today);
            (0..n).rev().map(|i| this - Duration::weeks(i)).collect()
        }
        View::Month => {
            let this = today.with_day(1).unwrap_or(today);
            (0..n as i32).rev().map(|i| add_months(this, -i)).collect()
        }
    }
}

/// Earliest instant a `view` chart looks at.
pub fn chart_start(view: View, now: DateTime<Local>) -> DateTime<Utc> {
    bucket_starts(view, now.date_naive())
        .first()
        .map(|d| local_midnight(*d))
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

/// Sum `tool`'s rows into the buckets of `view`.
pub fn columns(
    data: &Data,
    tool: ToolFilter,
    view: View,
    include_cache: bool,
    now: DateTime<Local>,
) -> Vec<Column> {
    let starts = bucket_starts(view, now.date_naive());
    let bounds: Vec<DateTime<Utc>> = starts.iter().map(|d| local_midnight(*d)).collect();
    let mut cols: Vec<Column> = starts
        .iter()
        .map(|d| {
            let (label, sub) = match view {
                View::Day => (
                    d.format("%d").to_string(),
                    Some(d.format("%a").to_string().chars().take(2).collect()),
                ),
                View::Week => (format!("W{:02}", d.iso_week().week()), None),
                View::Month => (d.format("%b").to_string(), None),
            };
            Column {
                start: *d,
                label,
                sub,
                tokens: 0,
                cost_usd: 0.0,
            }
        })
        .collect();
    let Some(first) = bounds.first().copied() else {
        return cols;
    };
    for r in data
        .rows
        .iter()
        .filter(|r| r.ts >= first && tool.includes(r.tool))
    {
        // Last bucket whose start is <= ts.
        let i = bounds.partition_point(|b| *b <= r.ts).saturating_sub(1);
        cols[i].tokens += row_tokens(r, include_cache);
        cols[i].cost_usd += r.cost;
    }
    cols
}

const PARTIAL: [char; 7] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/// Draw `cols` as a column chart `height` rows tall, with an optional budget line.
/// The last column is the current period (marked with `▲`). `fmt` formats axis values.
pub fn render(
    title: &str,
    cols: &[Column],
    metric: Metric,
    budget: Option<f64>,
    height: usize,
    fmt: impl Fn(f64) -> String,
) -> String {
    let height = height.max(2);
    let values: Vec<f64> = cols.iter().map(|c| c.value(metric).max(0.0)).collect();
    let budget = budget.filter(|b| b.is_finite() && *b > 0.0);
    let max = values.iter().copied().chain(budget).fold(0.0_f64, f64::max);

    let mut out = String::from(title);
    if let Some(b) = budget {
        out.push_str(&format!("  ┄ budget {}", fmt(b)));
    }
    out.push('\n');
    if cols.is_empty() || max <= 0.0 {
        out.push_str("(no usage)\n");
        return out;
    }

    let width = cols
        .iter()
        .flat_map(|c| {
            [
                c.label.chars().count(),
                c.sub.as_deref().map_or(0, |s| s.chars().count()),
            ]
        })
        .max()
        .unwrap_or(2)
        .max(2);
    let top = fmt(max);
    let mid = fmt(max / 2.0);
    let axis_w = top.chars().count().max(mid.chars().count());
    let budget_row =
        budget.map(|b| ((b / max * height as f64).ceil() as usize).clamp(1, height) - 1);

    for row in (0..height).rev() {
        let label = if row == height - 1 {
            top.as_str()
        } else if row == height / 2 - 1 && height >= 4 {
            mid.as_str()
        } else {
            ""
        };
        let mut line = format!(" {label:>axis_w$} │");
        let on_budget = budget_row == Some(row);
        for v in &values {
            let fill = v / max * height as f64;
            let cell = if fill >= (row + 1) as f64 {
                '█'
            } else if fill > row as f64 {
                let eighths = (((fill - row as f64) * 8.0).round() as usize).clamp(1, 7);
                PARTIAL[eighths - 1]
            } else if on_budget {
                '┄'
            } else {
                ' '
            };
            line.extend(std::iter::repeat_n(cell, width));
            line.push(if on_budget { '┄' } else { ' ' });
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    let pad = " ".repeat(axis_w + 2);
    let span = (width + 1) * cols.len();
    out.push_str(&format!("{pad}└{}\n", "─".repeat(span)));
    let row_of = |f: &dyn Fn(&Column) -> String| -> String {
        let cells: String = cols.iter().map(|c| format!("{:<width$} ", f(c))).collect();
        format!("{pad} {}", cells.trim_end())
    };
    out.push_str(&row_of(&|c| c.label.clone()));
    out.push('\n');
    if cols.iter().any(|c| c.sub.is_some()) {
        out.push_str(&row_of(&|c| c.sub.clone().unwrap_or_default()));
        out.push('\n');
    }
    let marker = format!("{pad} {}▲", " ".repeat((width + 1) * (cols.len() - 1)));
    out.push_str(&marker);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::tests::{now, row};
    use tokenburn_core::Tool;

    #[test]
    fn view_and_metric_parse() {
        for v in View::ALL {
            assert_eq!(v.as_str().parse::<View>().unwrap(), v);
        }
        assert_eq!("Monthly".parse::<View>().unwrap(), View::Month);
        assert!("year".parse::<View>().is_err());
        assert_eq!("usd".parse::<Metric>().unwrap(), Metric::Cost);
        assert!("joules".parse::<Metric>().is_err());
        assert_eq!(View::Week.range(), "last 8 weeks");
    }

    #[test]
    fn buckets_end_with_the_current_period() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap(); // Wednesday
        let days = bucket_starts(View::Day, today);
        assert_eq!(days.len(), 14);
        assert_eq!(days.last(), Some(&today));
        assert_eq!(days[0].to_string(), "2026-04-23");
        let weeks = bucket_starts(View::Week, today);
        assert_eq!(weeks.last().unwrap().to_string(), "2026-05-04");
        assert_eq!(weeks[0].to_string(), "2026-03-16");
        let months = bucket_starts(View::Month, today);
        assert_eq!(
            months.iter().map(|d| d.to_string()).collect::<Vec<_>>(),
            [
                "2025-12-01",
                "2026-01-01",
                "2026-02-01",
                "2026-03-01",
                "2026-04-01",
                "2026-05-01"
            ]
        );
    }

    #[test]
    fn rows_land_in_their_bucket_and_old_rows_are_ignored() {
        let n = now();
        let d = Data {
            rows: vec![
                row(Tool::Pi, "a", n - Duration::days(100), 999, 9.0),
                row(Tool::Pi, "a", n - Duration::days(1), 10, 1.0),
                row(Tool::Pi, "a", n - Duration::minutes(5), 20, 2.0),
                row(Tool::Claude, "b", n - Duration::minutes(5), 40, 4.0),
            ],
            ..Default::default()
        };
        let cols = columns(&d, ToolFilter::PI, View::Day, false, n);
        assert_eq!(cols.len(), 14);
        assert_eq!(cols[13].tokens, 30, "20 in + 10 out");
        assert_eq!(cols[12].tokens, 15);
        assert_eq!(cols.iter().map(|c| c.tokens).sum::<u64>(), 45);
        assert!((cols[13].cost_usd - 2.0).abs() < 1e-9);
        assert_eq!(cols[13].sub.as_deref(), Some("We"));
        let all = columns(&d, ToolFilter::All, View::Month, false, n);
        assert_eq!(all.last().unwrap().tokens, 15 + 30 + 60);
    }

    #[test]
    fn render_draws_bars_axis_labels_marker_and_budget() {
        let n = now();
        let d = Data {
            rows: vec![
                row(Tool::Pi, "a", n - Duration::days(1), 100, 0.0),
                row(Tool::Pi, "a", n - Duration::minutes(5), 200, 0.0),
            ],
            ..Default::default()
        };
        let cols = columns(&d, ToolFilter::All, View::Day, false, n);
        let s = render("Daily tokens", &cols, Metric::Tokens, Some(250.0), 8, |v| {
            format!("{v:.0}")
        });
        assert!(s.starts_with("Daily tokens  ┄ budget 250\n"), "{s}");
        assert!(s.contains(" 300 │"), "top of the axis is the max:\n{s}");
        assert!(s.contains('┄'), "budget line:\n{s}");
        assert!(s.contains('█'));
        assert!(s.contains("└"));
        assert!(s.trim_end().ends_with('▲'));
        assert!(s.contains("We"), "weekday row:\n{s}");
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 1 + 8 + 1 + 2 + 1, "{s}");
    }

    #[test]
    fn render_without_usage_says_so() {
        let cols = columns(&Data::default(), ToolFilter::All, View::Week, true, now());
        let s = render("Weekly", &cols, Metric::Cost, None, 8, |v| format!("{v}"));
        assert_eq!(s, "Weekly\n(no usage)\n");
        assert_eq!(
            render("x", &[], Metric::Tokens, None, 8, |v| format!("{v}")),
            "x\n(no usage)\n"
        );
    }

    #[test]
    fn partial_cells_use_eighth_blocks() {
        let col = |tokens| Column {
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            label: "01".into(),
            sub: None,
            tokens,
            cost_usd: 0.0,
        };
        let s = render(
            "t",
            &[col(100), col(50 + 6)],
            Metric::Tokens,
            None,
            2,
            |v| format!("{v}"),
        );
        // 56/100 of 2 rows = 1.12 rows: one full block and a ▁ on top.
        assert!(s.contains('▁'), "{s}");
    }
}
