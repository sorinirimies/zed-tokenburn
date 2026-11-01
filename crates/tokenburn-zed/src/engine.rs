//! One refresh: read the logs, compute the status-bar line and the new alerts.
//! Shared by the language server (every `refresh_secs`) and `tokenburn-zed status`.

use chrono::{DateTime, Local, Utc};
use tokenburn_core::{Level, Period, Settings};

use crate::budget::{self, AlertPaths, Notice};
use crate::config::Config;
use crate::stats::{self, Data};
use crate::status;

/// The result of one refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// Text after `TokenBurn: ` in the status bar; `None` while paused.
    pub status: Option<String>,
    /// Worst budget level of the status period.
    pub level: Level,
    /// Alerts raised for the first time by this refresh.
    pub notices: Vec<Notice>,
    /// Non-fatal read problems.
    pub warnings: Vec<String>,
}

impl Snapshot {
    /// The snapshot of a paused (disabled) status bar.
    pub fn paused() -> Self {
        Self {
            status: None,
            level: Level::Ok,
            notices: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// How far back one refresh has to read: the status period, and every period
/// that has a budget or limit.
pub fn load_start(
    cfg: &Config,
    limits: &tokenburn_core::Limits,
    now: DateTime<Local>,
) -> DateTime<Utc> {
    let b = &cfg.budgets;
    let budgeted = [
        (
            Period::Day,
            b.day_tokens.is_some() || limits.daily.is_some(),
        ),
        (
            Period::Week,
            b.week_tokens.is_some() || limits.weekly.is_some(),
        ),
        (
            Period::Month,
            b.month_tokens.is_some() || limits.monthly.is_some(),
        ),
    ];
    budgeted
        .iter()
        .filter(|(_, on)| *on)
        .map(|(p, _)| p.start(now))
        .chain(std::iter::once(cfg.period.start(now)))
        .min()
        .unwrap_or_else(|| cfg.period.start(now))
}

/// Compute a snapshot from already loaded data (pure apart from the alert logs).
pub fn compute(
    data: &Data,
    cfg: &Config,
    shared: &Settings,
    now: DateTime<Local>,
    paths: &AlertPaths,
) -> Snapshot {
    if !cfg.enabled {
        return Snapshot::paused();
    }
    let cur = cfg.currency(shared);
    let limits = cfg.limits(&shared.limits);
    let stats = stats::span_stats(
        data,
        cfg.tool,
        cfg.period,
        now,
        cfg.include_cache,
        shared.input_price(),
    );
    let (statuses, notices) = budget::check(
        data,
        cfg,
        &limits,
        &cur,
        now,
        paths,
        cfg.desktop_notifications,
    );
    let level = budget::span_level(&statuses, cfg.period);
    Snapshot {
        status: Some(status::line(&stats, cfg, level, &cur)),
        level,
        notices: if cfg.notify { notices } else { Vec::new() },
        warnings: data.warnings.clone(),
    }
}

/// Read the logs and compute, with the real clock, settings and alert logs.
pub fn refresh(cfg: &Config) -> Snapshot {
    if !cfg.enabled {
        return Snapshot::paused();
    }
    let shared = tokenburn_core::settings::load();
    let now = Local::now();
    let limits = cfg.limits(&shared.limits);
    let data = stats::load(load_start(cfg, &limits, now));
    compute(&data, cfg, &shared, now, &AlertPaths::live())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;
    use crate::stats::tests::{now, row};
    use chrono::Duration;
    use serde_json::json;
    use tokenburn_core::{Limits, Tool};

    #[test]
    fn load_start_covers_the_status_period_and_every_budget() {
        let n = now();
        let today = Config::default();
        assert_eq!(
            load_start(&today, &Limits::default(), n),
            Span::Today.start(n)
        );
        let monthly_limit = Limits {
            monthly: Some(100.0),
            ..Default::default()
        };
        assert_eq!(load_start(&today, &monthly_limit, n), Span::Month.start(n));
        let week_tokens = Config::from_layers([&json!({ "budgets": { "week_tokens": 5 } })]);
        assert_eq!(
            load_start(&week_tokens, &Limits::default(), n),
            Span::Week.start(n)
        );
        let all = Config::from_layers([&json!({ "period": "all" })]);
        assert_eq!(load_start(&all, &monthly_limit, n), Span::All.start(n));
    }

    #[test]
    fn a_disabled_config_reads_nothing_and_shows_nothing() {
        let cfg = Config::from_layers([&json!({ "enabled": false })]);
        assert_eq!(refresh(&cfg), Snapshot::paused());
    }

    #[test]
    fn compute_builds_the_line_level_and_notices() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AlertPaths {
            cost: Some(dir.path().join("a.json")),
            tokens: Some(dir.path().join("z.json")),
        };
        let n = now();
        let data = Data {
            rows: vec![row(Tool::Pi, "a", n - Duration::hours(1), 1000, 1.0)],
            warnings: vec!["w".into()],
            ..Default::default()
        };
        let cfg = Config::from_layers([&json!({ "budgets": { "day_tokens": 10 } })]);
        let snap = compute(&data, &cfg, &Settings::default(), n, &paths);
        assert_eq!(snap.level, Level::Over);
        let line = snap.status.clone().unwrap();
        assert!(
            line.starts_with("🔥 OVER BUDGET 3.5k ($1.00) [today]"),
            "{line}"
        );
        assert_eq!(snap.notices.len(), 1);
        assert_eq!(snap.warnings, ["w"]);
        // notify: false keeps the level but drops the pop-ups.
        let quiet =
            Config::from_layers([&json!({ "notify": false, "budgets": { "day_tokens": 10 } })]);
        let dir2 = tempfile::tempdir().unwrap();
        let paths2 = AlertPaths {
            tokens: Some(dir2.path().join("z.json")),
            ..Default::default()
        };
        let snap = compute(&data, &quiet, &Settings::default(), n, &paths2);
        assert_eq!(snap.level, Level::Over);
        assert!(snap.notices.is_empty());
    }
}
