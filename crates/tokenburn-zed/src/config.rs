//! Settings of the Zed front-end.
//!
//! They come from Zed's `settings.json`:
//!
//! ```jsonc
//! "lsp": {
//!   "tokenburn": {
//!     "settings": { "period": "today", "show_rate": true, "budgets": { "day_tokens": 5000000 } }
//!   }
//! }
//! ```
//!
//! (`initialization_options` are read the same way, `settings` win). The MCP
//! server gets the same object through the `TOKENBURN_ZED_SETTINGS` environment
//! variable. Parsing is lenient on purpose: a wrong value for one key falls back to
//! that key's default instead of throwing the whole object away, so a typo can
//! never take the status bar down.
//!
//! Display currency, the input price used to value savings and the dollar limits
//! are shared with every other TokenBurn front-end (`tokenburn-core`'s
//! `settings.json`) unless overridden here.

use serde::Serialize;
use serde_json::Value;
use tokenburn_core::{Currency, ToolFilter};

use crate::span::Span;

/// Environment variable carrying the settings object (JSON) to `tokenburn-zed mcp`.
pub const SETTINGS_ENV: &str = "TOKENBURN_ZED_SETTINGS";

/// Allowed refresh interval, seconds.
pub const MIN_REFRESH_SECS: u64 = 2;
pub const MAX_REFRESH_SECS: u64 = 3600;
pub const DEFAULT_REFRESH_SECS: u64 = 10;

/// Token and dollar budgets per period. Dollar ones default to the shared
/// TokenBurn limits (`settings.json` → `limits`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Budgets {
    pub day_tokens: Option<u64>,
    pub week_tokens: Option<u64>,
    pub month_tokens: Option<u64>,
    pub day_cost: Option<f64>,
    pub week_cost: Option<f64>,
    pub month_cost: Option<f64>,
}

impl Budgets {
    fn apply(&mut self, v: &Value) {
        let tokens = |k: &str| v.get(k).and_then(positive_u64);
        let cost = |k: &str| v.get(k).and_then(positive_f64);
        if v.get("day_tokens").is_some() {
            self.day_tokens = tokens("day_tokens");
        }
        if v.get("week_tokens").is_some() {
            self.week_tokens = tokens("week_tokens");
        }
        if v.get("month_tokens").is_some() {
            self.month_tokens = tokens("month_tokens");
        }
        if v.get("day_cost").is_some() {
            self.day_cost = cost("day_cost");
        }
        if v.get("week_cost").is_some() {
            self.week_cost = cost("week_cost");
        }
        if v.get("month_cost").is_some() {
            self.month_cost = cost("month_cost");
        }
    }

    /// Any token budget set?
    pub fn has_tokens(&self) -> bool {
        self.day_tokens.is_some() || self.week_tokens.is_some() || self.month_tokens.is_some()
    }
}

/// Everything the status bar, the alerts and the report can be told.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Config {
    /// Master switch: off clears the status bar and reads nothing in the background.
    pub enabled: bool,
    /// What the status bar covers.
    pub period: Span,
    /// Which tools count (`all`, `pi`, `zed`, `claude`, …).
    #[serde(serialize_with = "ser_tool")]
    pub tool: ToolFilter,
    /// Seconds between two refreshes.
    pub refresh_secs: u64,
    /// Count cache read/write tokens in the totals (like the `tokenburn` CLI).
    pub include_cache: bool,
    pub show_cost: bool,
    /// Tokens per hour over the period.
    pub show_rate: bool,
    /// Prompt-cache hit ratio.
    pub show_hit: bool,
    /// Estimated savings (prompt cache, rtk, pi compaction).
    pub show_saved: bool,
    /// Display currency code; `None` = the shared TokenBurn setting.
    pub currency: Option<String>,
    pub budgets: Budgets,
    /// Warn at this percent of a budget; `None` = the shared `limits.warn_at`.
    pub warn_pct: Option<f64>,
    /// Pop up a Zed notification when a budget/limit is nearly reached or exceeded.
    pub notify: bool,
    /// Also send an OS desktop notification for the shared dollar limits.
    pub desktop_notifications: bool,
    /// Offer the TokenBurn code actions (report, cycle period, pause).
    pub code_actions: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            period: Span::Today,
            tool: ToolFilter::All,
            refresh_secs: DEFAULT_REFRESH_SECS,
            include_cache: true,
            show_cost: true,
            show_rate: true,
            show_hit: true,
            show_saved: true,
            currency: None,
            budgets: Budgets::default(),
            warn_pct: None,
            notify: true,
            desktop_notifications: false,
            code_actions: true,
        }
    }
}

fn ser_tool<S: serde::Serializer>(t: &ToolFilter, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(t.as_str())
}

fn positive_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n
            .as_u64()
            .or_else(|| {
                n.as_f64()
                    .filter(|f| f.is_finite() && *f >= 1.0)
                    .map(|f| f as u64)
            })
            .filter(|n| *n > 0),
        _ => None,
    }
}

fn positive_f64(v: &Value) -> Option<f64> {
    v.as_f64().filter(|f| f.is_finite() && *f > 0.0)
}

impl Config {
    /// Defaults, overlaid with every object in `layers` in order (later wins).
    pub fn from_layers<'a>(layers: impl IntoIterator<Item = &'a Value>) -> Self {
        let mut c = Config::default();
        for layer in layers {
            c.apply(layer);
        }
        c
    }

    /// Overlay the keys present in `v` (anything but an object is ignored; so is
    /// a key whose value has the wrong type or is out of range).
    pub fn apply(&mut self, v: &Value) {
        // Zed hands over `lsp.tokenburn.settings`; accept a `tokenburn` wrapper too.
        let v = match v.get("tokenburn") {
            Some(inner) if inner.is_object() => inner,
            _ => v,
        };
        if !v.is_object() {
            return;
        }
        let flag = |k: &str| v.get(k).and_then(Value::as_bool);
        if let Some(b) = flag("enabled") {
            self.enabled = b;
        }
        if let Some(p) = v
            .get("period")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok())
        {
            self.period = p;
        }
        if let Some(t) = v
            .get("tool")
            .and_then(Value::as_str)
            .and_then(|s| s.parse().ok())
        {
            self.tool = t;
        }
        if let Some(n) = v.get("refresh_secs").and_then(positive_u64) {
            self.refresh_secs = n.clamp(MIN_REFRESH_SECS, MAX_REFRESH_SECS);
        }
        if let Some(b) = flag("include_cache") {
            self.include_cache = b;
        }
        if let Some(b) = flag("show_cost") {
            self.show_cost = b;
        }
        if let Some(b) = flag("show_rate") {
            self.show_rate = b;
        }
        if let Some(b) = flag("show_hit") {
            self.show_hit = b;
        }
        if let Some(b) = flag("show_saved") {
            self.show_saved = b;
        }
        match v.get("currency") {
            Some(Value::String(code)) if tokenburn_core::currency_by_code(code).is_some() => {
                self.currency = Some(code.trim().to_ascii_uppercase());
            }
            Some(Value::Null) => self.currency = None,
            _ => {}
        }
        if let Some(b) = v.get("budgets").filter(|b| b.is_object()) {
            self.budgets.apply(b);
        }
        match v.get("warn_pct") {
            Some(Value::Null) => self.warn_pct = None,
            Some(x) => {
                if let Some(p) = x
                    .as_f64()
                    .filter(|p| p.is_finite() && *p > 0.0 && *p <= 100.0)
                {
                    self.warn_pct = Some(p);
                }
            }
            None => {}
        }
        if let Some(b) = flag("notify") {
            self.notify = b;
        }
        if let Some(b) = flag("desktop_notifications") {
            self.desktop_notifications = b;
        }
        if let Some(b) = flag("code_actions") {
            self.code_actions = b;
        }
    }

    /// Settings handed to the MCP server through [`SETTINGS_ENV`].
    pub fn from_env() -> Self {
        let layer = std::env::var(SETTINGS_ENV)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .unwrap_or(Value::Null);
        Config::from_layers([&layer])
    }

    /// The display currency: this config's, else the shared TokenBurn one.
    pub fn currency(&self, shared: &tokenburn_core::Settings) -> Currency {
        let code = self.currency.as_deref().unwrap_or(&shared.currency);
        tokenburn_core::features::currency::resolve(code, &shared.rates)
    }

    /// Warning threshold as a fraction (0–1].
    pub fn warn_fraction(&self, shared: &tokenburn_core::Limits) -> f64 {
        self.warn_pct
            .map(|p| p / 100.0)
            .unwrap_or_else(|| shared.warn_fraction())
    }

    /// The shared dollar limits with this config's overrides applied.
    pub fn limits(&self, shared: &tokenburn_core::Limits) -> tokenburn_core::Limits {
        let mut l = shared.clone();
        if self.budgets.day_cost.is_some() {
            l.daily = self.budgets.day_cost;
        }
        if self.budgets.week_cost.is_some() {
            l.weekly = self.budgets.week_cost;
        }
        if self.budgets.month_cost.is_some() {
            l.monthly = self.budgets.month_cost;
        }
        l.warn_at = self.warn_fraction(shared);
        l
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokenburn_core::Tool;

    #[test]
    fn defaults_match_the_documented_ones() {
        let c = Config::default();
        assert!(c.enabled && c.include_cache && c.show_rate && c.show_hit && c.show_saved);
        assert_eq!(c.period, Span::Today);
        assert_eq!(c.tool, ToolFilter::All);
        assert_eq!(c.refresh_secs, DEFAULT_REFRESH_SECS);
        assert!(!c.desktop_notifications);
    }

    #[test]
    fn every_key_is_read() {
        let c = Config::from_layers([&json!({
            "enabled": false, "period": "week", "tool": "claude", "refresh_secs": 30,
            "include_cache": false, "show_cost": false, "show_rate": false, "show_hit": false,
            "show_saved": false, "currency": "eur", "warn_pct": 90, "notify": false,
            "desktop_notifications": true, "code_actions": false,
            "budgets": { "day_tokens": 5_000_000, "week_cost": 50.5 }
        })]);
        assert!(!c.enabled && !c.include_cache && !c.show_cost && !c.show_rate);
        assert!(!c.show_hit && !c.show_saved && !c.notify && !c.code_actions);
        assert!(c.desktop_notifications);
        assert_eq!(c.period, Span::Week);
        assert_eq!(c.tool, ToolFilter::Only(Tool::Claude));
        assert_eq!(c.refresh_secs, 30);
        assert_eq!(c.currency.as_deref(), Some("EUR"));
        assert_eq!(c.warn_pct, Some(90.0));
        assert_eq!(c.budgets.day_tokens, Some(5_000_000));
        assert_eq!(c.budgets.week_cost, Some(50.5));
        assert!(c.budgets.has_tokens());
    }

    #[test]
    fn a_bad_value_keeps_the_default_for_that_key_only() {
        let c = Config::from_layers([&json!({
            "period": "fortnight", "tool": "notepad", "refresh_secs": "fast",
            "currency": "XYZ", "warn_pct": 250, "show_rate": "yes", "show_hit": false,
            "budgets": { "day_tokens": -5, "month_cost": "lots" }
        })]);
        assert_eq!(c.period, Span::Today);
        assert_eq!(c.tool, ToolFilter::All);
        assert_eq!(c.refresh_secs, DEFAULT_REFRESH_SECS);
        assert_eq!(c.currency, None);
        assert_eq!(c.warn_pct, None);
        assert!(c.show_rate, "a non-boolean is ignored");
        assert!(!c.show_hit, "…while the valid key next to it still applies");
        assert_eq!(c.budgets, Budgets::default());
    }

    #[test]
    fn non_objects_are_ignored() {
        for v in [json!(null), json!(42), json!("today"), json!([1, 2])] {
            assert_eq!(Config::from_layers([&v]), Config::default());
        }
    }

    #[test]
    fn refresh_is_clamped() {
        let fast = Config::from_layers([&json!({ "refresh_secs": 1 })]);
        let slow = Config::from_layers([&json!({ "refresh_secs": 999_999 })]);
        assert_eq!(fast.refresh_secs, MIN_REFRESH_SECS);
        assert_eq!(slow.refresh_secs, MAX_REFRESH_SECS);
    }

    #[test]
    fn later_layers_win_and_null_clears() {
        let init = json!({ "period": "month", "currency": "EUR", "budgets": { "day_tokens": 10 } });
        let settings =
            json!({ "period": "year", "currency": null, "budgets": { "day_tokens": null } });
        let c = Config::from_layers([&init, &settings]);
        assert_eq!(c.period, Span::Year);
        assert_eq!(c.currency, None);
        assert_eq!(c.budgets.day_tokens, None);
    }

    #[test]
    fn a_tokenburn_wrapper_is_unwrapped() {
        let c = Config::from_layers([&json!({ "tokenburn": { "period": "all" } })]);
        assert_eq!(c.period, Span::All);
    }

    #[test]
    fn cost_budgets_override_the_shared_limits() {
        let shared = tokenburn_core::Limits {
            daily: Some(20.0),
            weekly: Some(100.0),
            ..Default::default()
        };
        let c = Config::from_layers([&json!({ "budgets": { "day_cost": 5 }, "warn_pct": 50 })]);
        let l = c.limits(&shared);
        assert_eq!(l.daily, Some(5.0));
        assert_eq!(l.weekly, Some(100.0), "unset keys keep the shared limit");
        assert_eq!(l.monthly, None);
        assert!((l.warn_at - 0.5).abs() < 1e-9);
        assert!((Config::default().warn_fraction(&shared) - 0.8).abs() < 1e-9);
    }

    #[test]
    fn currency_falls_back_to_the_shared_setting() {
        let shared = tokenburn_core::Settings {
            currency: "GBP".into(),
            ..Default::default()
        };
        assert_eq!(Config::default().currency(&shared).code, "GBP");
        let own = Config::from_layers([&json!({ "currency": "JPY" })]);
        assert_eq!(own.currency(&shared).code, "JPY");
    }
}
