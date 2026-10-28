//! One "how healthy is this phone" number (0-100) and the before/after chart. The score turns raw
//! diagnostics into something a person can act on; the chart shows what the run actually changed.

use crate::diagnostics::Diag;
use crate::ui;

/// Each junk app found costs this many points, up to JUNK_MAX_PENALTY.
const JUNK_POINTS: f64 = 0.5;
const JUNK_MAX_PENALTY: f64 = 20.0;

fn storage_penalty(d: &Diag) -> f64 {
    match d.storage.as_ref().map(|s| s.pct).unwrap_or(0) {
        p if p >= 90 => 30.0,
        p if p >= 85 => 20.0,
        p if p >= 75 => 8.0,
        _ => 0.0,
    }
}

fn ram_penalty(d: &Diag) -> f64 {
    if d.ram_total_mb <= 0 {
        return 0.0;
    }
    match 100 * d.ram_free_mb / d.ram_total_mb {
        f if f < 10 => 20.0,
        f if f < 20 => 10.0,
        f if f < 30 => 3.0,
        _ => 0.0,
    }
}

fn battery_penalty(d: &Diag) -> f64 {
    let failing = if d.battery_health != 0 && d.battery_health != 2 { 15.0 } else { 0.0 };
    let worn = if (1..80).contains(&d.battery_capacity_pct) {
        ((80 - d.battery_capacity_pct) as f64 / 2.0).min(15.0)
    } else {
        0.0
    };
    failing + worn
}

fn heat_and_keys_penalty(d: &Diag) -> f64 {
    let heat = match d.thermal_status {
        t if t >= 3 => 15.0,
        2 => 8.0,
        1 => 3.0,
        _ => 0.0,
    };
    heat + if d.stuck_volume_key.is_some() { 15.0 } else { 0.0 }
}

/// 0-100, higher is healthier. `junk` is how many ads/junk packages are still installed.
pub fn score(d: &Diag, junk: usize) -> i32 {
    let penalty = storage_penalty(d)
        + ram_penalty(d)
        + battery_penalty(d)
        + heat_and_keys_penalty(d)
        + (junk as f64 * JUNK_POINTS).min(JUNK_MAX_PENALTY);
    (100.0 - penalty).clamp(0.0, 100.0).round() as i32
}

pub fn label(score: i32) -> &'static str {
    match score {
        90.. => "excellent",
        75..=89 => "good",
        55..=74 => "fair",
        35..=54 => "poor",
        _ => "bad",
    }
}

/// One point in time, condensed for the chart.
pub struct Measure {
    pub score: i32,
    pub launch_ms: Option<i64>,
    pub ram_free_mb: i64,
    pub ram_total_mb: i64,
    pub storage_free_mb: i64,
    pub storage_total_mb: i64,
    pub packages: usize,
}

impl Measure {
    pub fn new(d: &Diag, junk: usize, launch_ms: Option<i64>) -> Measure {
        Measure {
            score: score(d, junk),
            launch_ms,
            ram_free_mb: d.ram_free_mb,
            ram_total_mb: d.ram_total_mb,
            storage_free_mb: d.storage.as_ref().map(|s| s.free_mb).unwrap_or(0),
            storage_total_mb: d.storage.as_ref().map(|s| s.total_mb).unwrap_or(0),
            packages: d.packages,
        }
    }
}

fn chart_row(title: &str, before: i64, after: i64, max: i64, show: &dyn Fn(i64) -> String) {
    if max <= 0 {
        return;
    }
    ui::say(title);
    ui::dim(&format!("before {} {}", ui::bar(before as f64 / max as f64), show(before)));
    ui::dim(&format!("after  {} {}", ui::bar(after as f64 / max as f64), show(after)));
}

pub fn show_chart(before: &Measure, after: &Measure) {
    ui::step("Results");
    let points = |v: i64| format!("{v}/100 ({})", label(v as i32));
    chart_row("Health score", before.score as i64, after.score as i64, 100, &points);
    if let (Some(b), Some(a)) = (before.launch_ms, after.launch_ms) {
        let ms = |v: i64| format!("{v} ms");
        chart_row("App start time (shorter is faster)", b, a, b.max(a), &ms);
    }
    chart_row("Free RAM", before.ram_free_mb, after.ram_free_mb, before.ram_total_mb, &ui::format_mb);
    chart_row("Free storage", before.storage_free_mb, after.storage_free_mb, before.storage_total_mb, &ui::format_mb);
    ui::dim(&format!("Apps in the system: {} -> {}", before.packages, after.packages));
}
