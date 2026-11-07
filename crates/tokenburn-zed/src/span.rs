//! The time spans the status bar and the report talk about.
//!
//! `tokenburn-core` knows today / this month / all history (for its report) and
//! day / week / month (for spending limits). The status bar of pi-tokenburn also
//! offers the rolling last hour and the calendar year, so the Zed front-end has its
//! own, wider set. Every boundary is local time; weeks start on Monday.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tokenburn_core::Period;

/// What the status bar (and a `period` argument) covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Span {
    /// The rolling last 60 minutes.
    Hour,
    /// Since local midnight.
    #[default]
    Today,
    /// Since Monday 00:00.
    Week,
    /// Since the 1st of the month.
    Month,
    /// Since January 1st.
    Year,
    /// Everything on disk.
    All,
}

impl Span {
    /// Every span, shortest first.
    pub const ALL: [Span; 6] = [
        Span::Hour,
        Span::Today,
        Span::Week,
        Span::Month,
        Span::Year,
        Span::All,
    ];

    /// Machine name, also what the settings accept.
    pub fn as_str(self) -> &'static str {
        match self {
            Span::Hour => "hour",
            Span::Today => "today",
            Span::Week => "week",
            Span::Month => "month",
            Span::Year => "year",
            Span::All => "all",
        }
    }

    /// Human label for headings: `last hour`, `today`, `this week`, …
    pub fn title(self) -> &'static str {
        match self {
            Span::Hour => "last hour",
            Span::Today => "today",
            Span::Week => "this week",
            Span::Month => "this month",
            Span::Year => "this year",
            Span::All => "all time",
        }
    }

    /// The spans the code actions offer for the status bar, in menu order. The
    /// last hour stays a setting only.
    pub const PICKS: [Span; 5] = [Span::Today, Span::Week, Span::Month, Span::Year, Span::All];

    /// First instant of the span containing `now`, as UTC.
    pub fn start(self, now: DateTime<Local>) -> DateTime<Utc> {
        let today = now.date_naive();
        match self {
            Span::Hour => (now - Duration::hours(1)).with_timezone(&Utc),
            Span::Today => local_midnight(today),
            Span::Week => local_midnight(week_start(today)),
            Span::Month => local_midnight(today.with_day(1).unwrap_or(today)),
            Span::Year => {
                local_midnight(NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today))
            }
            Span::All => DateTime::<Utc>::UNIX_EPOCH,
        }
    }

    /// The spending-limit period that applies to this span, if any: the last
    /// hour and today share the daily limit; year and all time have none.
    pub fn period(self) -> Option<Period> {
        match self {
            Span::Hour | Span::Today => Some(Period::Day),
            Span::Week => Some(Period::Week),
            Span::Month => Some(Period::Month),
            Span::Year | Span::All => None,
        }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Span {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "hour" | "hourly" | "1h" => Ok(Span::Hour),
            "today" | "day" | "daily" => Ok(Span::Today),
            "week" | "weekly" => Ok(Span::Week),
            "month" | "monthly" => Ok(Span::Month),
            "year" | "yearly" => Ok(Span::Year),
            "all" | "total" => Ok(Span::All),
            other => Err(format!(
                "period must be hour, today, week, month, year or all (got '{other}')"
            )),
        }
    }
}

/// Monday of the week containing `day`.
pub fn week_start(day: NaiveDate) -> NaiveDate {
    day - Duration::days(i64::from(day.weekday().num_days_from_monday()))
}

/// Local midnight at the start of `day`, as UTC (the earliest one on a DST change;
/// the Unix epoch if the local time does not exist at all).
pub fn local_midnight(day: NaiveDate) -> DateTime<Utc> {
    day.and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(y, m, d, h, 30, 0).unwrap()
    }

    #[test]
    fn span_round_trips_through_its_name() {
        for s in Span::ALL {
            assert_eq!(s.as_str().parse::<Span>().unwrap(), s);
        }
        assert_eq!("DAY".parse::<Span>().unwrap(), Span::Today);
        assert_eq!(" total ".parse::<Span>().unwrap(), Span::All);
        assert!("fortnight".parse::<Span>().is_err());
    }

    #[test]
    fn the_menu_offers_day_week_month_year_and_all() {
        let names: Vec<&str> = Span::PICKS.iter().map(|s| s.as_str()).collect();
        assert_eq!(names, ["today", "week", "month", "year", "all"]);
    }

    // 2026-05-06 is a Wednesday.
    #[test]
    fn starts_are_local_calendar_boundaries() {
        let now = at(2026, 5, 6, 15);
        let local = |s: Span| s.start(now).with_timezone(&Local);
        assert_eq!(
            local(Span::Today),
            at(2026, 5, 6, 0) - Duration::minutes(30)
        );
        assert_eq!(local(Span::Week).date_naive().to_string(), "2026-05-04");
        assert_eq!(local(Span::Month).date_naive().to_string(), "2026-05-01");
        assert_eq!(local(Span::Year).date_naive().to_string(), "2026-01-01");
        assert_eq!(Span::All.start(now), DateTime::<Utc>::UNIX_EPOCH);
        assert_eq!(
            Span::Hour.start(now),
            (now - Duration::hours(1)).with_timezone(&Utc)
        );
    }

    #[test]
    fn a_monday_is_its_own_week_start() {
        let monday = NaiveDate::from_ymd_opt(2026, 5, 4).unwrap();
        assert_eq!(week_start(monday), monday);
        let sunday = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
        assert_eq!(week_start(sunday), monday);
    }

    #[test]
    fn only_calendar_spans_up_to_a_month_have_a_limit_period() {
        assert_eq!(Span::Hour.period(), Some(Period::Day));
        assert_eq!(Span::Today.period(), Some(Period::Day));
        assert_eq!(Span::Week.period(), Some(Period::Week));
        assert_eq!(Span::Month.period(), Some(Period::Month));
        assert_eq!(Span::Year.period(), None);
        assert_eq!(Span::All.period(), None);
    }
}
