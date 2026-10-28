//! Before and after in words: for every measurement, is it better, worse or the same - and why, from what
//! the run actually did. Differences smaller than the measurement noise count as "about the same".

use crate::ui::chart::{Measure, Row, Verdict};
use crate::ui::format_mb;

#[derive(Clone, Copy)]
pub enum Direction {
    HigherIsBetter,
    LowerIsBetter,
}

/// What the run did, so each change can be explained.
pub struct RunFacts {
    pub removed: usize,
    pub compiled: bool,
}

/// App start times vary by a few percent within a run but by up to 10-15% between moments of the day
/// (measured on a real phone), so smaller differences count as "about the same".
const SPEED_NOISE: f64 = 0.10;
/// Free RAM moves with whatever runs at the moment.
const RAM_NOISE: f64 = 0.05;
const STORAGE_NOISE: f64 = 0.01;
const SCORE_NOISE: f64 = 1.0;

pub fn compare(direction: Direction, before: f64, after: f64, noise: f64) -> Verdict {
    let gain = match direction {
        Direction::HigherIsBetter => after - before,
        Direction::LowerIsBetter => before - after,
    };
    if gain.abs() <= noise {
        Verdict::Same
    } else if gain > 0.0 {
        Verdict::Better
    } else {
        Verdict::Worse
    }
}

fn fraction(value: f64, max: f64) -> f64 {
    if max > 0.0 {
        value / max
    } else {
        0.0
    }
}

fn score_row(b: &Measure, a: &Measure, facts: &RunFacts) -> Row {
    let verdict = compare(Direction::HigherIsBetter, b.score as f64, a.score as f64, SCORE_NOISE);
    let reason = match verdict {
        Verdict::Better if facts.removed > 0 => format!("{} junk apps removed", facts.removed),
        Verdict::Worse => "the phone is warmer or busier than before".into(),
        _ => String::new(),
    };
    let text = |m: &Measure| (m.score as f64 / 100.0, format!("{}/100 ({})", m.score, m.score_label));
    Row { title: "Health score".into(), before: text(b), after: text(a), verdict, reason }
}

fn speed_rows(b: &Measure, a: &Measure) -> Vec<Row> {
    b.launches
        .iter()
        .filter_map(|(name, before)| {
            let (_, after) = a.launches.iter().find(|(n, _)| n == name)?;
            let (before, after) = (*before as f64, *after as f64);
            let verdict = compare(Direction::LowerIsBetter, before, after, before * SPEED_NOISE);
            let change = (100.0 * (before - after).abs() / before).round();
            let reason = match verdict {
                Verdict::Better => format!("{change}% faster"),
                Verdict::Worse => format!("{change}% slower"),
                _ => "within measurement noise".into(),
            };
            let max = before.max(after);
            Some(Row {
                title: format!("{name} start time (shorter is faster)"),
                before: (fraction(before, max), format!("{before} ms")),
                after: (fraction(after, max), format!("{after} ms")),
                verdict,
                reason,
            })
        })
        .collect()
}

fn compiled_row(b: &Measure, a: &Measure) -> Row {
    let (bc, bt) = b.compiled_icon_apps;
    let (ac, at) = a.compiled_icon_apps;
    let verdict = compare(Direction::HigherIsBetter, bc as f64, ac as f64, 0.0);
    let reason =
        if verdict == Verdict::Better { "they start without compiling on the fly".into() } else { String::new() };
    Row {
        title: "Apps with an icon compiled".into(),
        before: (fraction(bc as f64, bt as f64), format!("{bc} of {bt}")),
        after: (fraction(ac as f64, at as f64), format!("{ac} of {at}")),
        verdict,
        reason,
    }
}

fn ram_row(b: &Measure, a: &Measure) -> Row {
    let total = b.ram_total_mb as f64;
    let verdict = compare(Direction::HigherIsBetter, b.ram_free_mb as f64, a.ram_free_mb as f64, total * RAM_NOISE);
    let reason = match verdict {
        Verdict::Better => "less runs in the background; it fills up again as you use apps".into(),
        _ => String::new(),
    };
    Row {
        title: "Free RAM".into(),
        before: (fraction(b.ram_free_mb as f64, total), format_mb(b.ram_free_mb)),
        after: (fraction(a.ram_free_mb as f64, total), format_mb(a.ram_free_mb)),
        verdict,
        reason,
    }
}

fn storage_row(b: &Measure, a: &Measure, facts: &RunFacts) -> Row {
    let total = b.storage_total_mb as f64;
    let raw =
        compare(Direction::HigherIsBetter, b.storage_free_mb as f64, a.storage_free_mb as f64, total * STORAGE_NOISE);
    let (verdict, reason) = match raw {
        Verdict::Worse if facts.compiled => {
            (Verdict::Expected, "compiled app code takes space - the price of faster apps".to_string())
        }
        Verdict::Better => (raw, "caches cleared".to_string()),
        _ => (raw, String::new()),
    };
    Row {
        title: "Free storage".into(),
        before: (fraction(b.storage_free_mb as f64, total), format_mb(b.storage_free_mb)),
        after: (fraction(a.storage_free_mb as f64, total), format_mb(a.storage_free_mb)),
        verdict,
        reason,
    }
}

fn apps_row(b: &Measure, a: &Measure, facts: &RunFacts) -> Row {
    let verdict = compare(Direction::LowerIsBetter, b.packages as f64, a.packages as f64, 0.0);
    let reason = if verdict == Verdict::Better && facts.removed > 0 {
        format!("{} junk apps removed", facts.removed)
    } else {
        String::new()
    };
    let max = b.packages.max(a.packages) as f64;
    Row {
        title: "Apps in the system".into(),
        before: (fraction(b.packages as f64, max), b.packages.to_string()),
        after: (fraction(a.packages as f64, max), a.packages.to_string()),
        verdict,
        reason,
    }
}

pub fn rows(before: &Measure, after: &Measure, facts: &RunFacts) -> Vec<Row> {
    let mut rows = vec![score_row(before, after, facts)];
    rows.extend(speed_rows(before, after));
    rows.push(compiled_row(before, after));
    rows.push(ram_row(before, after));
    rows.push(storage_row(before, after, facts));
    rows.push(apps_row(before, after, facts));
    rows
}
