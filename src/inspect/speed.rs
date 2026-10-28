//! How fast apps start from cold. Measured only on harmless system apps (never on the person's own apps,
//! which could do something when opened), through their home-screen activity, after a warm-up, as the
//! median of several runs - and "after" only once the phone has cooled down.

use super::hardware::cpu_busy;
use crate::phone::adb::Adb;
use crate::util::num_after;
use std::thread::sleep;
use std::time::{Duration, Instant};

const RUNS: usize = 5;
const PAUSE: Duration = Duration::from_millis(800);
const COOL_DOWN_MAX: Duration = Duration::from_secs(180);
const COOL_DOWN_POLL: Duration = Duration::from_secs(10);
const IDLE_CPU_PERCENT: i64 = 25;

/// Harmless system apps, found by what they are rather than by package name, so this works on any brand.
const PROBES: &[(&str, &str)] = &[
    ("Settings", "-a android.settings.SETTINGS"),
    ("Calendar", "-a android.intent.action.MAIN -c android.intent.category.APP_CALENDAR"),
    ("Clock", "-a android.intent.action.SHOW_ALARMS"),
    ("Calculator", "-a android.intent.action.MAIN -c android.intent.category.APP_CALCULATOR"),
];

pub struct Probe {
    pub name: &'static str,
    pub package: String,
    pub component: String,
}

/// The "pkg/.Activity" line of `cmd package resolve-activity --brief` output -> (package, component).
pub fn parse_component(resolved: &str) -> Option<(String, String)> {
    let line = resolved.lines().rev().map(str::trim).find(|l| l.contains('/') && !l.contains(' '))?;
    let (pkg, _) = line.split_once('/')?;
    Some((pkg.to_string(), line.to_string()))
}

pub fn median(values: &[i64]) -> Option<i64> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(sorted.len() / 2).copied()
}

fn resolve(adb: &Adb, intent: &str) -> Option<(String, String)> {
    parse_component(&adb.sh(&format!("cmd package resolve-activity --brief {intent}")))
        .filter(|(pkg, _)| pkg != "android") // a chooser, not an app
}

/// The probe apps this phone has, each with its home-screen activity.
pub fn probes(adb: &Adb) -> Vec<Probe> {
    PROBES
        .iter()
        .filter_map(|(name, intent)| {
            let (package, _) = resolve(adb, intent)?;
            let launcher = format!("-a android.intent.action.MAIN -c android.intent.category.LAUNCHER -p {package}");
            let (_, component) = resolve(adb, &launcher)?;
            Some(Probe { name, package, component })
        })
        .collect()
}

/// Cold-start time in ms: one warm-up start, then the median of RUNS starts. Opens the app briefly.
pub fn launch_ms(adb: &Adb, probe: &Probe) -> Option<i64> {
    let cold_start = || {
        let _ = adb.sh(&format!("am force-stop {}", probe.package));
        sleep(PAUSE);
        let out = adb.sh(&format!("am start -W -n {}", probe.component));
        let _ = adb.sh("input keyevent KEYCODE_HOME");
        num_after(&out, "TotalTime:").or_else(|| num_after(&out, "WaitTime:"))
    };
    let _warm_up = cold_start();
    let runs: Vec<i64> = (0..RUNS).filter_map(|_| cold_start()).collect();
    let _ = adb.sh(&format!("am force-stop {}", probe.package));
    median(&runs)
}

/// Wait until the phone is not throttling and the CPU is mostly idle. False if it took too long.
pub fn wait_until_cool(adb: &Adb) -> bool {
    let deadline = Instant::now() + COOL_DOWN_MAX;
    loop {
        let thermal = num_after(&adb.sh("dumpsys thermalservice"), "Thermal Status:").unwrap_or(0);
        if thermal == 0 && cpu_busy(adb).unwrap_or(0) <= IDLE_CPU_PERCENT {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        sleep(COOL_DOWN_POLL);
    }
}
