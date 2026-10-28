//! Bars, stage progress and the before/after chart.

use super::style::{paint, Tone};
use super::{format_mb, say, step};

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

/// One point in time, condensed for the chart.
pub struct Measure {
    pub score: i32,
    pub score_label: &'static str,
    pub launch_ms: Option<i64>,
    pub ram_free_mb: i64,
    pub ram_total_mb: i64,
    pub storage_free_mb: i64,
    pub storage_total_mb: i64,
    pub packages: usize,
}

fn row(title: &str, before: (i64, String), after: (i64, String), max: i64) {
    if max <= 0 {
        return;
    }
    say(&paint(title, Tone::Strong));
    let line = |label: &str, (value, text): (i64, String), tone: Tone| {
        say(&format!("  {label} {} {text}", paint(&bar(value as f64 / max as f64), tone)));
    };
    line("before", before, Tone::Before);
    line("after ", after, Tone::After);
}

pub fn show_chart(before: &Measure, after: &Measure) {
    step("Results");
    let score = |m: &Measure| (m.score as i64, format!("{}/100 ({})", m.score, m.score_label));
    row("Health score", score(before), score(after), 100);
    if let (Some(b), Some(a)) = (before.launch_ms, after.launch_ms) {
        row("App start time (shorter is faster)", (b, format!("{b} ms")), (a, format!("{a} ms")), b.max(a));
    }
    let mb = |v: i64| (v, format_mb(v));
    row("Free RAM", mb(before.ram_free_mb), mb(after.ram_free_mb), before.ram_total_mb);
    row("Free storage", mb(before.storage_free_mb), mb(after.storage_free_mb), before.storage_total_mb);
    say(&paint(&format!("Apps in the system: {} -> {}", before.packages, after.packages), Tone::Muted));
}
