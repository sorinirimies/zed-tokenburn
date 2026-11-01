//! The column chart never panics and always has the same shape: title, `height`
//! rows, axis, labels, weekday row, marker — or "(no usage)".
#![no_main]

use chrono::NaiveDate;
use libfuzzer_sys::fuzz_target;
use tokenburn_zed::chart::{render, Column, Metric};

fuzz_target!(|input: (Vec<(u32, f32)>, Option<f32>, u8)| {
    let (values, budget, height) = input;
    let start = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
    let cols: Vec<Column> = values
        .iter()
        .take(40)
        .map(|(t, c)| Column {
            start,
            label: "01".into(),
            sub: Some("Mo".into()),
            tokens: u64::from(*t),
            cost_usd: f64::from(*c),
        })
        .collect();
    let height = usize::from(height % 24);
    for metric in [Metric::Tokens, Metric::Cost] {
        let s = render("t", &cols, metric, budget.map(f64::from), height, |v| format!("{v:.1}"));
        let lines = s.lines().count();
        if s.contains("(no usage)") {
            assert_eq!(lines, 2);
        } else {
            assert_eq!(lines, 1 + height.max(2) + 1 + 2 + 1, "{s}");
        }
    }
});
