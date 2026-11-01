//! Markdown views: the full report (opened in a Zed tab by the code action, and
//! returned by the `tokenburn_report` MCP tool), plus the smaller pieces the other
//! MCP tools return.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};

use chrono::{DateTime, Local};
use serde::Serialize;
use tokenburn_core::{Currency, Limits, Row, Summary, Tool, ToolFilter};

use crate::budget::{self, BudgetStatus};
use crate::chart::{self, Metric, View};
use crate::config::Config;
use crate::fmt;
use crate::span::Span;
use crate::stats::{self, Data, SpanStats};

/// What a breakdown groups by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    #[default]
    Tool,
    /// Project / workspace (the model name for tools that log no project, e.g. Zed).
    Project,
}

impl FromStr for Group {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tool" | "tools" => Ok(Group::Tool),
            "project" | "projects" | "model" | "models" => Ok(Group::Project),
            other => Err(format!("by must be tool or project (got '{other}')")),
        }
    }
}

/// One line of a breakdown.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entry {
    pub name: String,
    pub summary: Summary,
    pub tokens: u64,
}

/// Usage of `span` grouped by `group`, biggest first.
pub fn breakdown(
    data: &Data,
    tool: ToolFilter,
    span: Span,
    group: Group,
    include_cache: bool,
    now: DateTime<Local>,
) -> Vec<Entry> {
    let start = span.start(now);
    let mut groups: HashMap<String, Vec<&Row>> = HashMap::new();
    for r in data
        .rows
        .iter()
        .filter(|r| r.ts >= start && tool.includes(r.tool))
    {
        let key = match group {
            Group::Tool => r.tool.display_name().to_string(),
            Group::Project => format!("{}:{}", r.tool.as_str(), project_name(r)),
        };
        groups.entry(key).or_default().push(r);
    }
    let mut out: Vec<Entry> = groups
        .into_iter()
        .map(|(key, rows)| {
            let summary = Summary::of(rows.iter().copied());
            Entry {
                name: key,
                tokens: stats::tokens_of(&summary, include_cache),
                summary,
            }
        })
        .collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens).then_with(|| a.name.cmp(&b.name)));
    out
}

/// A readable project name: the last directory of a path, the project directory
/// behind pi's / Claude Code's dash-encoded working directories
/// (`--Users-me-Projects-dodai-ui--` → `dodai-ui`), and anything else (model
/// names, for Zed and friends) unchanged.
pub fn project_name(r: &Row) -> String {
    let p = r.project.as_str();
    if p.contains(['/', '\\']) {
        return last_component(p);
    }
    if !matches!(r.tool, Tool::Pi | Tool::Claude) {
        return p.to_string();
    }
    static CACHE: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(Default::default);
    if let Some(hit) = CACHE.lock().ok().and_then(|c| c.get(p).cloned()) {
        return hit;
    }
    let home = dirs::home_dir();
    let name = decode_dashed(p, Path::new("/"))
        .map(|d| last_component(&d.to_string_lossy()))
        .unwrap_or_else(|| strip_encoded_prefix(p, home.as_deref()));
    if let Ok(mut c) = CACHE.lock() {
        c.insert(p.to_string(), name.clone());
    }
    name
}

fn last_component(p: &str) -> String {
    p.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(p)
        .to_string()
}

/// Rebuild the directory a dash-encoded name came from by walking the real file
/// system from `root`, taking the longest existing directory at each step (so
/// `dodai-ui` stays one name). `None` when the path no longer exists.
pub fn decode_dashed(encoded: &str, root: &Path) -> Option<PathBuf> {
    let parts: Vec<&str> = encoded.split('-').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return None;
    }
    let mut cur = root.to_path_buf();
    let mut i = 0;
    while i < parts.len() {
        let (j, next) = (i + 1..=parts.len())
            .rev()
            .map(|j| (j, cur.join(parts[i..j].join("-"))))
            .find(|(_, cand)| cand.is_dir())?;
        cur = next;
        i = j;
    }
    Some(cur)
}

/// Fallback when the directory is gone: drop the encoded home directory
/// (`Users-me-`), keep the rest (`Projects-dodai-ui`).
fn strip_encoded_prefix(p: &str, home: Option<&Path>) -> String {
    let trimmed = p.trim_matches('-');
    let Some(home) = home else {
        return trimmed.to_string();
    };
    let enc: String = home
        .to_string_lossy()
        .split(['/', '\\'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    match trimmed.strip_prefix(&enc) {
        Some(rest) if !rest.trim_matches('-').is_empty() => rest.trim_matches('-').to_string(),
        _ => trimmed.to_string(),
    }
}

fn escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn hit(s: &Summary) -> String {
    stats::hit_ratio(s).map_or("–".into(), fmt::percent)
}

/// A markdown table of a breakdown (the first `top` entries, the rest summed up).
pub fn breakdown_table(entries: &[Entry], top: usize, group: Group, cur: &Currency) -> String {
    if entries.is_empty() {
        return "_No usage in this period._\n".into();
    }
    let total: u64 = entries.iter().map(|e| e.tokens).sum();
    let head = match group {
        Group::Tool => "Tool",
        Group::Project => "Project",
    };
    let mut out = format!(
        "| {head} | Tokens | Input | Output | Cache hit | Cost | Share |\n|---|--:|--:|--:|--:|--:|--:|\n"
    );
    let line = |name: &str, s: &Summary, tokens: u64| {
        format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            escape(name),
            fmt::tokens(tokens),
            fmt::tokens(s.input),
            fmt::tokens(s.output),
            hit(s),
            cur.format(s.cost_usd),
            fmt::share(tokens, total)
        )
    };
    let top = top.max(1);
    for e in entries.iter().take(top) {
        out.push_str(&line(&e.name, &e.summary, e.tokens));
    }
    if entries.len() > top {
        let rest = &entries[top..];
        let mut s = Summary::default();
        for e in rest {
            s.input += e.summary.input;
            s.output += e.summary.output;
            s.cache_read += e.summary.cache_read;
            s.cache_write += e.summary.cache_write;
            s.cost_usd += e.summary.cost_usd;
        }
        let tokens = rest.iter().map(|e| e.tokens).sum();
        out.push_str(&line(&format!("{} more", rest.len()), &s, tokens));
    }
    out
}

/// Everything a report needs, computed once.
pub struct Context<'a> {
    pub data: &'a Data,
    pub cfg: &'a Config,
    pub cur: Currency,
    pub limits: Limits,
    pub input_price: f64,
    pub now: DateTime<Local>,
}

impl<'a> Context<'a> {
    /// Use the shared TokenBurn settings for currency, input price and limits.
    pub fn new(data: &'a Data, cfg: &'a Config, now: DateTime<Local>) -> Self {
        let shared = tokenburn_core::settings::load();
        Self {
            data,
            cfg,
            cur: cfg.currency(&shared),
            limits: cfg.limits(&shared.limits),
            input_price: shared.input_price(),
            now,
        }
    }

    pub fn stats(&self, span: Span) -> SpanStats {
        stats::span_stats(
            self.data,
            self.cfg.tool,
            span,
            self.now,
            self.cfg.include_cache,
            self.input_price,
        )
    }

    pub fn budgets(&self) -> Vec<BudgetStatus> {
        budget::statuses(self.data, self.cfg, &self.limits, self.now)
    }

    fn scope(&self) -> String {
        let tools = match self.cfg.tool {
            ToolFilter::All => "all tools".to_string(),
            ToolFilter::Only(t) => t.display_name().to_string(),
        };
        let cache = if self.cfg.include_cache {
            "cache tokens included"
        } else {
            "input + output only"
        };
        format!(
            "{tools} · {cache} · {}{}",
            self.cur.code,
            self.cur.approx_mark()
        )
    }
}

/// The usage table, one line per span.
pub fn usage_table(ctx: &Context, spans: &[Span]) -> String {
    let mut out = String::from(
        "| Period | Tokens | Input | Output | Cache read | Cache write | Turns | Cost | Cache hit | Saved | Rate |\n\
         |---|--:|--:|--:|--:|--:|--:|--:|--:|--:|--:|\n",
    );
    for &span in spans {
        let s = ctx.stats(span);
        let rate = if s.tokens > 0 {
            format!("{}/h", fmt::tokens(s.tokens_per_hour.round() as u64))
        } else {
            "–".into()
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            span.title(),
            fmt::tokens(s.tokens),
            fmt::tokens(s.summary.input),
            fmt::tokens(s.summary.output),
            fmt::tokens(s.summary.cache_read),
            fmt::tokens(s.summary.cache_write),
            s.summary.turns,
            ctx.cur.format(s.cost_usd),
            s.hit_ratio.map_or("–".into(), fmt::percent),
            ctx.cur.format(s.savings.usd),
            rate,
        ));
    }
    out
}

/// One span in a few lines (the `tokenburn_usage` MCP tool).
pub fn usage_text(ctx: &Context, span: Span) -> String {
    let s = ctx.stats(span);
    let mut out = format!("TokenBurn usage — {} ({})\n", span.title(), ctx.scope());
    out.push_str(&format!(
        "- tokens: {} (input {}, output {}, cache read {}, cache write {})\n",
        fmt::tokens(s.tokens),
        fmt::tokens(s.summary.input),
        fmt::tokens(s.summary.output),
        fmt::tokens(s.summary.cache_read),
        fmt::tokens(s.summary.cache_write)
    ));
    out.push_str(&format!("- turns: {}\n", s.summary.turns));
    out.push_str(&format!(
        "- cost: {} (logged by the tools; Zed logs none)\n",
        ctx.cur.format(s.cost_usd)
    ));
    out.push_str(&format!(
        "- burn rate: {}/h · {}/h\n",
        fmt::tokens(s.tokens_per_hour.round() as u64),
        ctx.cur.format(s.cost_per_hour)
    ));
    if let Some(h) = s.hit_ratio {
        out.push_str(&format!("- prompt cache hit ratio: {}\n", fmt::percent(h)));
    }
    out.push_str(&format!(
        "- estimated savings: {} ({} tokens)\n",
        ctx.cur.format(s.savings.usd),
        fmt::tokens(s.savings.tokens)
    ));
    out
}

/// Budgets and limits with their forecasts, or a hint how to set them.
pub fn budgets_text(ctx: &Context) -> String {
    let st = ctx.budgets();
    if st.is_empty() {
        return "_No budgets set._ Add `budgets` (`day_tokens`, `week_cost`, …) to the \
                `tokenburn` language-server settings, or dollar limits with \
                `tokenburn --limit-daily 20`.\n"
            .into();
    }
    let mut out = String::new();
    for s in &st {
        let mark = match s.level {
            tokenburn_core::Level::Over => "🔴",
            tokenburn_core::Level::Warn => "🟡",
            tokenburn_core::Level::Ok => "🟢",
        };
        out.push_str(&format!(
            "- {mark} {} {}\n",
            bar(s.ratio()),
            budget::forecast_line(s, &ctx.cur, ctx.now)
        ));
    }
    out
}

/// `▕██████░░░░▏` for a ratio (capped at 100 %).
pub fn bar(ratio: f64) -> String {
    const W: usize = 10;
    let filled = ((ratio.clamp(0.0, 1.0)) * W as f64).round() as usize;
    format!("`{}{}`", "█".repeat(filled), "░".repeat(W - filled))
}

/// A chart with its `Σ · avg · peak` footer, inside a code block.
pub fn chart_block(ctx: &Context, view: View, metric: Metric) -> String {
    let cols = chart::columns(ctx.data, ctx.cfg.tool, view, ctx.cfg.include_cache, ctx.now);
    let period = match view {
        View::Day => tokenburn_core::Period::Day,
        View::Week => tokenburn_core::Period::Week,
        View::Month => tokenburn_core::Period::Month,
    };
    let budget = ctx
        .budgets()
        .into_iter()
        .find(|s| {
            s.period == period
                && matches!(
                    (s.kind, metric),
                    (budget::Kind::Tokens, Metric::Tokens) | (budget::Kind::Cost, Metric::Cost)
                )
        })
        .map(|s| s.budget);
    let what = match metric {
        Metric::Tokens => "tokens",
        Metric::Cost => "cost",
    };
    let title = format!("{} {what} · {}", view.adjective(), view.range());
    let cur = ctx.cur;
    let fmt_v = |v: f64| match metric {
        Metric::Tokens => fmt::tokens(v.round() as u64),
        Metric::Cost => cur.format(v),
    };
    let mut body = chart::render(&title, &cols, metric, budget, 8, fmt_v);
    let tokens: u64 = cols.iter().map(|c| c.tokens).sum();
    let cost: f64 = cols.iter().map(|c| c.cost_usd).sum();
    if let Some(peak) = cols
        .iter()
        .filter(|c| c.value(metric) > 0.0)
        .max_by(|a, b| a.value(metric).total_cmp(&b.value(metric)))
    {
        let n = cols.len().max(1) as f64;
        let avg = match metric {
            Metric::Tokens => fmt::tokens((tokens as f64 / n).round() as u64),
            Metric::Cost => cur.format(cost / n),
        };
        let when = match view {
            View::Day => peak.start.format("%m-%d %a").to_string(),
            View::Week => format!("week of {}", peak.start.format("%m-%d")),
            View::Month => peak.start.format("%Y-%m").to_string(),
        };
        body.push_str(&format!(
            "Σ {} ({}) · avg {avg}/{} · peak {} ({when})\n",
            fmt::tokens(tokens),
            cur.format(cost),
            view.unit(),
            fmt_v(peak.value(metric)),
        ));
    }
    format!("```text\n{body}```\n")
}

/// Savings of a span, per source.
pub fn savings_table(ctx: &Context, span: Span) -> String {
    let s = ctx.stats(span).savings;
    if s.sources.is_empty() {
        return "_Nothing saved in this period._\n".into();
    }
    let mut out =
        String::from("| Source | Tokens | Estimated money | Events |\n|---|--:|--:|--:|\n");
    for src in &s.sources {
        out.push_str(&format!(
            "| {} | {} | {} | {} {} |\n",
            src.source.label(),
            fmt::tokens(src.tokens),
            ctx.cur.format(src.usd),
            src.events,
            src.source.event_noun()
        ));
    }
    out.push_str(&format!(
        "| **Total** | **{}** | **{}** | |\n",
        fmt::tokens(s.tokens),
        ctx.cur.format(s.usd)
    ));
    out
}

/// The full report.
pub fn full(ctx: &Context, breakdown_span: Span) -> String {
    let mut out = String::from("# 🔥 TokenBurn\n\n");
    out.push_str(&format!(
        "_{} · {}_\n\n",
        ctx.now.format("%Y-%m-%d %H:%M"),
        ctx.scope()
    ));
    out.push_str("## Usage\n\n");
    out.push_str(&usage_table(ctx, &Span::ALL));
    out.push_str("\n## Budgets and limits\n\n");
    out.push_str(&budgets_text(ctx));
    out.push_str("\n## Charts\n\n");
    for view in View::ALL {
        out.push_str(&chart_block(ctx, view, Metric::Tokens));
        out.push('\n');
    }
    out.push_str(&chart_block(ctx, View::Day, Metric::Cost));
    for (group, title) in [(Group::Tool, "By tool"), (Group::Project, "Top projects")] {
        out.push_str(&format!("\n## {title} — {}\n\n", breakdown_span.title()));
        let entries = breakdown(
            ctx.data,
            ctx.cfg.tool,
            breakdown_span,
            group,
            ctx.cfg.include_cache,
            ctx.now,
        );
        out.push_str(&breakdown_table(&entries, 10, group, &ctx.cur));
    }
    out.push_str(&format!("\n## Savings — {}\n\n", breakdown_span.title()));
    out.push_str(&savings_table(ctx, breakdown_span));
    out.push_str(&format!(
        "\nSavings are estimates at {}/M input tokens (`input_price_per_mtok` in TokenBurn's \
         `settings.json`). Costs are what the tools logged; Zed, Codex, Gemini, Copilot, Amp \
         and Qwen log none.\n",
        Currency::USD.format(ctx.input_price)
    ));
    if !ctx.data.warnings.is_empty() {
        out.push_str("\n## Warnings\n\n");
        for w in &ctx.data.warnings {
            out.push_str(&format!("- {}\n", escape(w)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::tests::{now, row};
    use chrono::Duration;
    use serde_json::json;
    use tokenburn_core::Tool;

    fn data() -> Data {
        let n = now();
        Data {
            rows: vec![
                row(
                    Tool::Pi,
                    "/Users/me/Projects/atlas",
                    n - Duration::hours(1),
                    1000,
                    2.0,
                ),
                row(
                    Tool::Pi,
                    "/Users/me/Projects/atlas",
                    n - Duration::days(2),
                    500,
                    1.0,
                ),
                row(
                    Tool::Claude,
                    "/Users/me/Projects/orbit",
                    n - Duration::hours(2),
                    200,
                    0.5,
                ),
                row(
                    Tool::Zed,
                    "claude-sonnet",
                    n - Duration::days(60),
                    9000,
                    0.0,
                ),
            ],
            warnings: vec!["zed: database is locked".into()],
            ..Default::default()
        }
    }

    fn ctx<'a>(d: &'a Data, c: &'a Config) -> Context<'a> {
        Context {
            data: d,
            cfg: c,
            cur: Currency::USD,
            limits: Limits::default(),
            input_price: 3.0,
            now: now(),
        }
    }

    #[test]
    fn group_parses() {
        assert_eq!("projects".parse::<Group>().unwrap(), Group::Project);
        assert_eq!("Tool".parse::<Group>().unwrap(), Group::Tool);
        assert!("color".parse::<Group>().is_err());
    }

    #[test]
    fn breakdown_groups_sorts_and_filters() {
        let d = data();
        let by_tool = breakdown(&d, ToolFilter::All, Span::Month, Group::Tool, false, now());
        assert_eq!(by_tool.len(), 2, "the Zed row is older than this month");
        assert_eq!(by_tool[0].name, "pi");
        assert_eq!(by_tool[0].summary.turns, 2);
        assert_eq!(by_tool[1].name, "Claude Code");
        let by_project = breakdown(&d, ToolFilter::All, Span::All, Group::Project, false, now());
        assert_eq!(by_project[0].name, "zed:claude-sonnet");
        assert!(by_project.iter().any(|e| e.name == "pi:atlas"));
        let only_claude = breakdown(
            &d,
            ToolFilter::Only(Tool::Claude),
            Span::All,
            Group::Tool,
            false,
            now(),
        );
        assert_eq!(only_claude.len(), 1);
    }

    #[test]
    fn breakdown_table_folds_the_tail() {
        let d = data();
        let e = breakdown(&d, ToolFilter::All, Span::All, Group::Project, false, now());
        let t = breakdown_table(&e, 1, Group::Project, &Currency::USD);
        assert!(t.starts_with("| Project | Tokens"));
        assert!(t.contains("| zed:claude-sonnet |"));
        assert!(t.contains("| 2 more |"), "{t}");
        assert_eq!(t.lines().count(), 4);
        assert_eq!(
            breakdown_table(&[], 5, Group::Tool, &Currency::USD),
            "_No usage in this period._\n"
        );
    }

    #[test]
    fn project_names_are_short_but_model_names_stay_whole() {
        let n = now();
        let name = |tool, p: &str| project_name(&row(tool, p, n, 1, 0.0));
        assert_eq!(name(Tool::Codex, "/home/me/code/orbit/"), "orbit");
        assert_eq!(name(Tool::Cline, "C:\\work\\site"), "site");
        assert_eq!(name(Tool::Zed, "claude-sonnet-4"), "claude-sonnet-4");
        // A gone directory falls back to the encoded name minus the home prefix.
        assert_eq!(
            strip_encoded_prefix(
                "--Users-me-Projects-gone-app--",
                Some(Path::new("/Users/me"))
            ),
            "Projects-gone-app"
        );
        assert_eq!(
            strip_encoded_prefix("--Users-me--", Some(Path::new("/Users/me"))),
            "Users-me"
        );
        assert_eq!(strip_encoded_prefix("-x-", None), "x");
    }

    #[test]
    fn dashed_names_are_decoded_against_the_file_system() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Users/me/Projects/dodai-ui")).unwrap();
        std::fs::create_dir_all(root.path().join("Users/me/Projects/dodai")).unwrap();
        let d = decode_dashed("--Users-me-Projects-dodai-ui--", root.path()).unwrap();
        assert!(d.ends_with("Projects/dodai-ui"), "{d:?}");
        let d = decode_dashed("Users-me-Projects-dodai", root.path()).unwrap();
        assert!(d.ends_with("Projects/dodai"));
        assert_eq!(
            decode_dashed("Users-me-Projects-missing", root.path()),
            None
        );
        assert_eq!(decode_dashed("---", root.path()), None);
    }

    #[test]
    fn pipes_in_names_do_not_break_tables() {
        assert_eq!(escape("a|b"), "a\\|b");
    }

    #[test]
    fn bar_is_capped() {
        assert_eq!(bar(0.0), "`░░░░░░░░░░`");
        assert_eq!(bar(0.5), "`█████░░░░░`");
        assert_eq!(bar(3.0), "`██████████`");
        assert_eq!(bar(f64::NAN.max(0.0)), "`░░░░░░░░░░`");
    }

    #[test]
    fn full_report_has_every_section() {
        let d = data();
        let c = Config::from_layers([&json!({ "budgets": { "day_tokens": 1000 } })]);
        let r = full(&ctx(&d, &c), Span::Month);
        for section in [
            "# 🔥 TokenBurn",
            "## Usage",
            "| last hour |",
            "| all time |",
            "## Budgets and limits",
            "daily token budget",
            "## Charts",
            "Daily tokens · last 14 days",
            "Weekly tokens · last 8 weeks",
            "Monthly tokens · last 6 months",
            "Daily cost · last 14 days",
            "## By tool — this month",
            "## Top projects — this month",
            "## Savings — this month",
            "## Warnings",
            "database is locked",
        ] {
            assert!(r.contains(section), "missing {section:?} in\n{r}");
        }
        assert_eq!(r.matches("```text").count(), 4);
    }

    #[test]
    fn report_without_budgets_explains_how_to_set_them() {
        let d = Data::default();
        let c = Config::default();
        let r = full(&ctx(&d, &c), Span::Month);
        assert!(r.contains("_No budgets set._"));
        assert!(r.contains("(no usage)"));
        assert!(!r.contains("## Warnings"));
    }

    #[test]
    fn usage_text_lists_the_numbers() {
        let d = data();
        let c = Config::default();
        let t = usage_text(&ctx(&d, &c), Span::Today);
        assert!(t.starts_with("TokenBurn usage — today (all tools"));
        assert!(t.contains("- turns: 2"));
        assert!(t.contains("- cost: $2.50"));
        assert!(t.contains("prompt cache hit ratio"));
    }

    #[test]
    fn chart_block_has_a_footer_with_the_peak() {
        let d = data();
        let c = Config::default();
        let b = chart_block(&ctx(&d, &c), View::Day, Metric::Tokens);
        assert!(b.starts_with("```text\nDaily tokens"));
        assert!(b.contains("Σ "), "{b}");
        assert!(b.contains("avg ") && b.contains("/day · peak "));
        assert!(b.ends_with("```\n"));
    }

    #[test]
    fn savings_table_lists_sources_and_total() {
        let d = data();
        let c = Config::default();
        let t = savings_table(&ctx(&d, &c), Span::All);
        assert!(t.contains("| Prompt cache |"), "{t}");
        assert!(t.contains("| **Total** |"));
        let empty = Data::default();
        assert_eq!(
            savings_table(&ctx(&empty, &c), Span::All),
            "_Nothing saved in this period._\n"
        );
    }
}
