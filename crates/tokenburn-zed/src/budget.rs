//! Budgets: how much of each daily / weekly / monthly budget is used, where the
//! current pace ends up, and the one-time alerts.
//!
//! Two kinds, like pi-tokenburn:
//!
//! * **Cost** — the shared TokenBurn spending limits (`settings.json` → `limits`,
//!   overridable per Zed settings). They always count every tool, and their alerts
//!   go through `tokenburn-core`'s shared alert log, so the CLI watcher, TUI, GUI,
//!   web server and Zed never alert twice for the same thing.
//! * **Tokens** — Zed-only budgets (`budgets.day_tokens`, …). They follow the tool
//!   filter and `include_cache`; their alerts are remembered in `zed-alerts.json`
//!   next to the shared `settings.json`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tokenburn_core::{Currency, Level, Limits, Period};

use crate::config::Config;
use crate::fmt;
use crate::span::{local_midnight, Span};
use crate::stats::{row_tokens, Data};

/// What a budget limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Tokens,
    Cost,
}

/// A linear forecast: pace so far × the whole period.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Projection {
    /// Expected use at the end of the period.
    pub projected: f64,
    /// When the budget runs out at this pace, if that is before the period ends.
    pub exhaust_at: Option<DateTime<Utc>>,
}

/// One budget against what its current period used.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BudgetStatus {
    pub period: Period,
    pub kind: Kind,
    pub used: f64,
    pub budget: f64,
    pub level: Level,
    pub projection: Option<Projection>,
}

impl BudgetStatus {
    /// Used as a fraction of the budget (may exceed 1).
    pub fn ratio(&self) -> f64 {
        self.used / self.budget
    }

    /// Whole percent used.
    pub fn percent(&self) -> u32 {
        (self.ratio() * 100.0).round().max(0.0) as u32
    }
}

/// A notification to show once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub level: Level,
    pub title: String,
    pub body: String,
}

/// First instant of the period after the one containing `now`.
pub fn period_end(period: Period, now: DateTime<Local>) -> DateTime<Utc> {
    let start = period.start(now).with_timezone(&Local).date_naive();
    let next = match period {
        Period::Day => start + Duration::days(1),
        Period::Week => start + Duration::days(7),
        Period::Month => {
            let (y, m) = if start.month() == 12 {
                (start.year() + 1, 1)
            } else {
                (start.year(), start.month() + 1)
            };
            NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(start + Duration::days(31))
        }
    };
    local_midnight(next)
}

/// Linear projection from `start` to `end`; `None` before anything was used.
pub fn project(
    used: f64,
    budget: f64,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Option<Projection> {
    let elapsed = (now - start).num_seconds() as f64;
    if !(used > 0.0 && elapsed > 0.0 && budget > 0.0) {
        return None;
    }
    let pace = used / elapsed; // per second
    let total = (end - start).num_seconds() as f64;
    let exhaust_at = (used < budget)
        .then(|| now + Duration::seconds(((budget - used) / pace).round() as i64))
        .filter(|at| *at < end);
    Some(Projection {
        projected: pace * total,
        exhaust_at,
    })
}

fn level(used: f64, budget: f64, warn: f64) -> Level {
    if used >= budget {
        Level::Over
    } else if used >= budget * warn {
        Level::Warn
    } else {
        Level::Ok
    }
}

fn token_budget(cfg: &Config, period: Period) -> Option<u64> {
    match period {
        Period::Day => cfg.budgets.day_tokens,
        Period::Week => cfg.budgets.week_tokens,
        Period::Month => cfg.budgets.month_tokens,
    }
}

/// Every configured budget, day → week → month, tokens before cost.
pub fn statuses(
    data: &Data,
    cfg: &Config,
    limits: &Limits,
    now: DateTime<Local>,
) -> Vec<BudgetStatus> {
    let warn = limits.warn_fraction();
    let now_utc = now.with_timezone(&Utc);
    let mut out = Vec::new();
    for period in Period::ALL {
        let start = period.start(now);
        let end = period_end(period, now);
        if let Some(budget) = token_budget(cfg, period) {
            let used: u64 = data
                .rows
                .iter()
                .filter(|r| r.ts >= start && cfg.tool.includes(r.tool))
                .map(|r| row_tokens(r, cfg.include_cache))
                .sum();
            let (used, budget) = (used as f64, budget as f64);
            out.push(BudgetStatus {
                period,
                kind: Kind::Tokens,
                used,
                budget,
                level: level(used, budget, warn),
                projection: project(used, budget, start, end, now_utc),
            });
        }
        if let Some(budget) = limits.get(period) {
            let used = tokenburn_core::spent(&data.rows, period, now);
            out.push(BudgetStatus {
                period,
                kind: Kind::Cost,
                used,
                budget,
                level: level(used, budget, warn),
                projection: project(used, budget, start, end, now_utc),
            });
        }
    }
    out
}

/// The worst level among the budgets of `span`'s period.
pub fn span_level(statuses: &[BudgetStatus], span: Span) -> Level {
    let Some(period) = span.period() else {
        return Level::Ok;
    };
    statuses
        .iter()
        .filter(|s| s.period == period)
        .map(|s| s.level)
        .max()
        .unwrap_or(Level::Ok)
}

/// `1.20M tokens` / `$4.10`.
pub fn amount(kind: Kind, v: f64, cur: &Currency) -> String {
    match kind {
        Kind::Tokens => format!("{} tokens", fmt::tokens(v as u64)),
        Kind::Cost => cur.format(v),
    }
}

/// `today 14:05`, `tomorrow 09:00` or `Thu 14:05`.
pub fn when(at: DateTime<Utc>, now: DateTime<Local>) -> String {
    let at = at.with_timezone(&Local);
    let days = (at.date_naive() - now.date_naive()).num_days();
    let day = match days {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        _ => at.format("%a").to_string(),
    };
    format!("{day} {}", at.format("%H:%M"))
}

/// `daily token budget: 62% of 5.00M tokens used; at this pace it runs out today 18:40`.
pub fn forecast_line(s: &BudgetStatus, cur: &Currency, now: DateTime<Local>) -> String {
    let kind = match s.kind {
        Kind::Tokens => "token budget",
        Kind::Cost => "spending limit",
    };
    let used = format!("{}% of {} used", s.percent(), amount(s.kind, s.budget, cur));
    let tail = match (s.level, s.projection) {
        (Level::Over, _) => " (over)".to_string(),
        (
            _,
            Some(Projection {
                exhaust_at: Some(at),
                ..
            }),
        ) => format!("; at this pace it runs out {}", when(at, now)),
        (_, Some(p)) => format!(
            "; on track ({} projected)",
            amount(s.kind, p.projected, cur)
        ),
        (_, None) => String::new(),
    };
    format!("{} {kind}: {used}{tail}", s.period.label())
}

/// Token-budget alerts already raised, by id
/// (`day:tokens:2026-05-06:warn`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenAlertLog {
    pub raised: BTreeSet<String>,
}

impl TokenAlertLog {
    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Best effort: an unwritable log only means an alert may repeat.
    pub fn save_to(&self, path: &Path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }
}

/// `zed-alerts.json` next to the shared TokenBurn `settings.json`.
pub fn token_alert_log_path() -> Option<PathBuf> {
    tokenburn_core::settings::config_dir().map(|d| d.join("zed-alerts.json"))
}

fn alert_id(s: &BudgetStatus, now: DateTime<Local>) -> String {
    let start = s.period.start(now).with_timezone(&Local).date_naive();
    let level = if s.level == Level::Over {
        "over"
    } else {
        "warn"
    };
    format!("{}:tokens:{start}:{level}", s.period.label())
}

/// New token-budget alerts (each id once), pruning ids of past periods.
pub fn pending_token_alerts(
    statuses: &[BudgetStatus],
    log: &mut TokenAlertLog,
    now: DateTime<Local>,
) -> Vec<BudgetStatus> {
    let current: Vec<String> = Period::ALL
        .iter()
        .map(|p| {
            format!(
                "{}:tokens:{}:",
                p.label(),
                p.start(now).with_timezone(&Local).date_naive()
            )
        })
        .collect();
    log.raised
        .retain(|id| current.iter().any(|prefix| id.starts_with(prefix.as_str())));
    statuses
        .iter()
        .filter(|s| s.kind == Kind::Tokens && s.level != Level::Ok)
        .filter(|s| log.raised.insert(alert_id(s, now)))
        .copied()
        .collect()
}

fn token_notice(s: &BudgetStatus, cur: &Currency) -> Notice {
    let title = match s.level {
        Level::Over => format!("TokenBurn: {} token budget exceeded", s.period.label()),
        _ => format!(
            "TokenBurn: {} token budget nearly reached",
            s.period.label()
        ),
    };
    Notice {
        level: s.level,
        title,
        body: format!(
            "Used {} of {} {} ({}%)",
            amount(Kind::Tokens, s.used, cur),
            amount(Kind::Tokens, s.budget, cur),
            s.period.span(),
            s.percent()
        ),
    }
}

/// Where the alert logs live (`None` = check without remembering: no alerts).
#[derive(Debug, Clone, Default)]
pub struct AlertPaths {
    /// The shared dollar-limit log (`tokenburn-core`'s `alerts.json`).
    pub cost: Option<PathBuf>,
    /// The Zed token-budget log.
    pub tokens: Option<PathBuf>,
}

impl AlertPaths {
    /// The real logs.
    pub fn live() -> Self {
        Self {
            cost: tokenburn_core::alert_log_path(),
            tokens: token_alert_log_path(),
        }
    }
}

/// Statuses plus the alerts that are new since the last check (remembered in the
/// logs at `paths`). Dollar-limit alerts are also sent as desktop notifications
/// when `desktop` is set and the shared limits allow it.
pub fn check(
    data: &Data,
    cfg: &Config,
    limits: &Limits,
    cur: &Currency,
    now: DateTime<Local>,
    paths: &AlertPaths,
    desktop: bool,
) -> (Vec<BudgetStatus>, Vec<Notice>) {
    let st = statuses(data, cfg, limits, now);
    let mut notices = Vec::new();

    if let Some(path) = &paths.tokens {
        let mut log = TokenAlertLog::load_from(path);
        let before = log.clone();
        let fresh = pending_token_alerts(&st, &mut log, now);
        if log != before {
            log.save_to(path);
        }
        notices.extend(fresh.iter().map(|s| token_notice(s, cur)));
    }

    let (_, cost_alerts) =
        tokenburn_core::check_limits_at(&data.rows, limits, now, paths.cost.as_deref());
    let money = |usd: f64| cur.format(usd);
    if desktop && limits.notify {
        tokenburn_core::notify_alerts(&cost_alerts, money);
    }
    notices.extend(cost_alerts.iter().map(|a| Notice {
        level: a.level,
        title: a.title(),
        body: a.body(money),
    }));
    (st, notices)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::tests::{now, row};
    use serde_json::json;
    use tokenburn_core::Tool;

    fn cfg(v: serde_json::Value) -> Config {
        Config::from_layers([&v])
    }

    fn data(rows: Vec<tokenburn_core::Row>) -> Data {
        Data {
            rows,
            ..Default::default()
        }
    }

    #[test]
    fn period_ends_are_the_next_local_boundary() {
        let n = now(); // Wed 2026-05-06 15:00
        let end = |p| {
            period_end(p, n)
                .with_timezone(&Local)
                .date_naive()
                .to_string()
        };
        assert_eq!(end(Period::Day), "2026-05-07");
        assert_eq!(end(Period::Week), "2026-05-11");
        assert_eq!(end(Period::Month), "2026-06-01");
        let december = Local.with_ymd_and_hms(2026, 12, 20, 9, 0, 0).unwrap();
        assert_eq!(
            period_end(Period::Month, december)
                .with_timezone(&Local)
                .date_naive()
                .to_string(),
            "2027-01-01"
        );
    }

    use chrono::TimeZone;

    #[test]
    fn projection_finds_when_the_budget_runs_out() {
        let start = Utc.with_ymd_and_hms(2026, 5, 6, 0, 0, 0).unwrap();
        let end = start + Duration::days(1);
        let now = start + Duration::hours(6);
        // 50 used in 6 h of a 100 budget: runs out after 12 h, at 12:00.
        let p = project(50.0, 100.0, start, end, now).unwrap();
        assert_eq!(p.exhaust_at, Some(start + Duration::hours(12)));
        assert!((p.projected - 200.0).abs() < 1e-9);
        // Slow pace: on track, no exhaustion inside the period.
        let p = project(10.0, 100.0, start, end, now).unwrap();
        assert_eq!(p.exhaust_at, None);
        // Already over: no exhaustion time either.
        assert_eq!(
            project(150.0, 100.0, start, end, now).unwrap().exhaust_at,
            None
        );
        // Nothing used yet, or a zero budget: no projection.
        assert_eq!(project(0.0, 100.0, start, end, now), None);
        assert_eq!(project(10.0, 0.0, start, end, now), None);
        assert_eq!(project(10.0, 100.0, start, end, start), None);
    }

    #[test]
    fn token_and_cost_budgets_get_levels() {
        let n = now();
        let d = data(vec![
            row(Tool::Pi, "a", n - Duration::hours(1), 1000, 9.0), // 1500 in+out, $9
            row(Tool::Claude, "b", n - Duration::days(3), 1000, 50.0),
        ]);
        let c = cfg(json!({
            "include_cache": false,
            "budgets": { "day_tokens": 1000, "week_tokens": 100000 }
        }));
        let limits = Limits {
            daily: Some(10.0),
            ..Default::default()
        };
        let st = statuses(&d, &c, &limits, n);
        let find = |p, k| st.iter().find(|s| s.period == p && s.kind == k).unwrap();
        assert_eq!(find(Period::Day, Kind::Tokens).level, Level::Over);
        assert_eq!(find(Period::Day, Kind::Tokens).used, 1500.0);
        assert_eq!(
            find(Period::Day, Kind::Cost).level,
            Level::Warn,
            "$9 of $10"
        );
        assert_eq!(find(Period::Week, Kind::Tokens).level, Level::Ok);
        assert_eq!(st.len(), 3, "no monthly budget, no weekly limit");
        assert_eq!(span_level(&st, Span::Today), Level::Over);
        assert_eq!(span_level(&st, Span::Hour), Level::Over);
        assert_eq!(span_level(&st, Span::Week), Level::Ok);
        assert_eq!(span_level(&st, Span::All), Level::Ok);
    }

    #[test]
    fn token_budgets_follow_the_tool_filter_but_cost_limits_do_not() {
        let n = now();
        let d = data(vec![row(
            Tool::Claude,
            "b",
            n - Duration::hours(1),
            1000,
            9.0,
        )]);
        let c = cfg(json!({ "tool": "pi", "budgets": { "day_tokens": 10 } }));
        let limits = Limits {
            daily: Some(10.0),
            ..Default::default()
        };
        let st = statuses(&d, &c, &limits, n);
        assert_eq!(st[0].kind, Kind::Tokens);
        assert_eq!(st[0].used, 0.0, "claude rows don't count for a pi filter");
        assert_eq!(st[1].used, 9.0, "the dollar limit counts every tool");
    }

    #[test]
    fn token_alerts_fire_once_per_level_and_period() {
        let n = now();
        let mut log = TokenAlertLog::default();
        let warn = BudgetStatus {
            period: Period::Day,
            kind: Kind::Tokens,
            used: 85.0,
            budget: 100.0,
            level: Level::Warn,
            projection: None,
        };
        assert_eq!(pending_token_alerts(&[warn], &mut log, n).len(), 1);
        assert!(
            pending_token_alerts(&[warn], &mut log, n).is_empty(),
            "only once"
        );
        let over = BudgetStatus {
            level: Level::Over,
            used: 120.0,
            ..warn
        };
        assert_eq!(
            pending_token_alerts(&[over], &mut log, n).len(),
            1,
            "over is new"
        );
        // Next day: yesterday's ids are pruned, the warning may fire again.
        let tomorrow = n + Duration::days(1);
        assert_eq!(pending_token_alerts(&[warn], &mut log, tomorrow).len(), 1);
        assert_eq!(log.raised.len(), 1);
        // Cost statuses and OK ones never produce token alerts.
        let cost = BudgetStatus {
            kind: Kind::Cost,
            ..over
        };
        let ok = BudgetStatus {
            level: Level::Ok,
            ..warn
        };
        assert!(pending_token_alerts(&[cost, ok], &mut log, tomorrow).is_empty());
    }

    #[test]
    fn check_remembers_alerts_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AlertPaths {
            cost: Some(dir.path().join("alerts.json")),
            tokens: Some(dir.path().join("zed-alerts.json")),
        };
        let n = now();
        let d = data(vec![row(Tool::Pi, "a", n - Duration::hours(1), 1000, 30.0)]);
        let c = cfg(json!({ "budgets": { "day_tokens": 100, "day_cost": 20 } }));
        let limits = c.limits(&Limits::default());
        let cur = Currency::USD;
        let (st, notices) = check(&d, &c, &limits, &cur, n, &paths, false);
        assert_eq!(st.len(), 2);
        assert_eq!(notices.len(), 2, "{notices:?}");
        assert!(notices.iter().all(|x| x.level == Level::Over));
        assert!(notices[0].title.contains("token budget exceeded"));
        assert!(notices[1].body.contains("$30.00"));
        let (_, again) = check(&d, &c, &limits, &cur, n, &paths, false);
        assert!(again.is_empty(), "both logs remember");
        // Without logs nothing is remembered, so nothing is raised.
        let (_, none) = check(&d, &c, &limits, &cur, n, &AlertPaths::default(), false);
        assert!(none.is_empty());
    }

    #[test]
    fn a_corrupt_log_is_treated_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("zed-alerts.json");
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(TokenAlertLog::load_from(&p), TokenAlertLog::default());
        assert_eq!(
            TokenAlertLog::load_from(&dir.path().join("missing.json")),
            TokenAlertLog::default()
        );
    }

    #[test]
    fn forecast_lines_read_naturally() {
        let n = now();
        let cur = Currency::USD;
        let mut s = BudgetStatus {
            period: Period::Day,
            kind: Kind::Tokens,
            used: 1_000_000.0,
            budget: 5_000_000.0,
            level: Level::Ok,
            projection: Some(Projection {
                projected: 2_000_000.0,
                exhaust_at: None,
            }),
        };
        assert_eq!(
            forecast_line(&s, &cur, n),
            "daily token budget: 20% of 5.00M tokens used; on track (2.00M tokens projected)"
        );
        s.projection = Some(Projection {
            projected: 9e6,
            exhaust_at: Some((n + Duration::hours(3)).with_timezone(&Utc)),
        });
        assert_eq!(
            forecast_line(&s, &cur, n),
            "daily token budget: 20% of 5.00M tokens used; at this pace it runs out today 18:00"
        );
        let over = BudgetStatus {
            kind: Kind::Cost,
            used: 12.0,
            budget: 10.0,
            level: Level::Over,
            projection: None,
            ..s
        };
        assert_eq!(
            forecast_line(&over, &cur, n),
            "daily spending limit: 120% of $10.00 used (over)"
        );
    }

    #[test]
    fn when_names_today_tomorrow_or_the_weekday() {
        let n = now(); // Wednesday 15:00
        let at = |h| (n + Duration::hours(h)).with_timezone(&Utc);
        assert_eq!(when(at(2), n), "today 17:00");
        assert_eq!(when(at(20), n), "tomorrow 11:00");
        assert_eq!(when(at(48), n), "Fri 15:00");
    }
}
