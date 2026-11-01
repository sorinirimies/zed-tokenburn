//! Any JSON is a valid settings object: parsing never panics, every value stays
//! in range, and applying the same layer twice changes nothing.
#![no_main]

use libfuzzer_sys::fuzz_target;
use tokenburn_zed::config::{Config, MAX_REFRESH_SECS, MIN_REFRESH_SECS};

fuzz_target!(|data: &[u8]| {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(data) else {
        return;
    };
    let once = Config::from_layers([&v]);
    let twice = Config::from_layers([&v, &v]);
    assert_eq!(once, twice, "applying a layer is idempotent");
    assert!((MIN_REFRESH_SECS..=MAX_REFRESH_SECS).contains(&once.refresh_secs));
    if let Some(p) = once.warn_pct {
        assert!(p > 0.0 && p <= 100.0);
    }
    let b = once.budgets;
    for t in [b.day_tokens, b.week_tokens, b.month_tokens].into_iter().flatten() {
        assert!(t > 0);
    }
    for c in [b.day_cost, b.week_cost, b.month_cost].into_iter().flatten() {
        assert!(c.is_finite() && c > 0.0);
    }
    // And it serialises back.
    serde_json::to_string(&once).unwrap();
});
