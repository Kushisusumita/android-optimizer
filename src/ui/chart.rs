//! Bars, stage progress, and the before/after results with a verdict in words for every row.

use super::style::{paint, Tone};
use super::{say, step};

const BAR_WIDTH: usize = 24;

/// A text bar for a 0..1 fraction: [#########---------------].
pub fn bar(fraction: f64) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * BAR_WIDTH as f64).round() as usize;
    format!("[{}{}]", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
}

/// Numbered stages of a long run, each header showing how far along the whole run is.
pub struct Progress {
    total: usize,
    started: usize,
}

impl Progress {
    pub fn new(total: usize) -> Progress {
        Progress { total, started: 0 }
    }

    pub fn next(&mut self, title: &str) {
        let done = self.started.min(self.total);
        self.started += 1;
        let total = self.total.max(1);
        step(&format!("[{}/{}] {title}", self.started, self.total));
        say(&paint(&format!("{} {}% done", bar(done as f64 / total as f64), 100 * done / total), Tone::Heading));
    }
}

/// One point in time, condensed for the results.
pub struct Measure {
    pub score: i32,
    pub score_label: &'static str,
    /// (app, cold-start ms) for each probe app that could be measured.
    pub launches: Vec<(String, i64)>,
    pub ram_free_mb: i64,
    pub ram_total_mb: i64,
    pub storage_free_mb: i64,
    pub storage_total_mb: i64,
    pub packages: usize,
    /// (compiled, total) apps with a home-screen icon.
    pub compiled_icon_apps: (usize, usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Better,
    Worse,
    Same,
    /// Worse on purpose: the price of something better (compiled code takes storage).
    Expected,
}

impl Verdict {
    fn word(self) -> &'static str {
        match self {
            Verdict::Better => "better",
            Verdict::Worse => "worse",
            Verdict::Same => "about the same",
            Verdict::Expected => "as expected",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Verdict::Better => Tone::Good,
            Verdict::Worse => Tone::Warning,
            Verdict::Same | Verdict::Expected => Tone::Muted,
        }
    }
}

/// One line of the results: two bars (fraction 0..1 and the value as text) and what it means.
pub struct Row {
    pub title: String,
    pub before: (f64, String),
    pub after: (f64, String),
    pub verdict: Verdict,
    pub reason: String,
}

pub fn show_results(rows: &[Row]) {
    step("Results");
    for r in rows {
        let reason = if r.reason.is_empty() { String::new() } else { format!(": {}", r.reason) };
        say(&format!("{} - {}{reason}", paint(&r.title, Tone::Strong), paint(r.verdict.word(), r.verdict.tone())));
        say(&format!("  before {} {}", paint(&bar(r.before.0), Tone::Before), r.before.1));
        say(&format!("  after  {} {}", paint(&bar(r.after.0), Tone::After), r.after.1));
    }
    let count = |v: Verdict| rows.iter().filter(|r| r.verdict == v).count();
    let summary = format!(
        "In short: {} better, {} about the same, {} as expected, {} worse",
        count(Verdict::Better),
        count(Verdict::Same),
        count(Verdict::Expected),
        count(Verdict::Worse)
    );
    step(&summary);
}
