//! One "how healthy is this phone" number (0-100): it turns raw diagnostics into something a person can
//! act on. Drawing it is the job of `ui::chart`.

use super::diagnostics::Diag;

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
