//! Cache cleanup, storage TRIM, ahead-of-time compilation of apps, and stopping background processes.

use crate::config;
use crate::inspect::diagnostics::{self, Diag};
use crate::phone::adb::Adb;
use crate::phone::info::DeviceInfo;
use crate::ui::{self, chart};

/// Runs on the phone: compiles apps with an icon, System UI and the home screen (see the script itself).
const COMPILE_SCRIPT: &str = include_str!("compile-apps.sh");
const COMPILE_REMOTE: &str = "/data/local/tmp/clearphone-compile.sh";
/// "Free everything" for `pm trim-caches`: 1 TiB.
const TRIM_ALL_BYTES: &str = "1099511627776";
/// Compiling on a nearly empty battery risks the phone dying mid-run.
const COMPILE_MIN_BATTERY: i64 = 20;
/// Android 7+ has the speed compiler this relies on.
const COMPILE_MIN_SDK: i32 = 24;

/// One line of compile-apps.sh output, "12/47 OK com.pkg" or "12/47 -- com.pkg".
pub struct CompileStep {
    pub index: usize,
    pub total: usize,
    pub ok: bool,
    pub package: String,
}

pub fn parse_compile_line(line: &str) -> Option<CompileStep> {
    let mut it = line.split_whitespace();
    let (index, total) = it.next()?.split_once('/')?;
    let ok = match it.next()? {
        "OK" => true,
        "--" => false,
        _ => return None,
    };
    Some(CompileStep { index: index.parse().ok()?, total: total.parse().ok()?, ok, package: it.next()?.to_string() })
}

pub fn clean_caches(adb: &Adb) {
    ui::step("Cache cleanup");
    let before = diagnostics::storage(adb);
    let _ = adb.sh(&format!("pm trim-caches {TRIM_ALL_BYTES}"));
    if let (Some(b), Some(a)) = (before, diagnostics::storage(adb)) {
        ui::ok(&format!("App caches cleared: {} freed", ui::format_mb((a.free_mb - b.free_mb).max(0))));
    }
    let _ = adb.sh("sm idle-maint run");
    ui::ok("Storage maintenance (TRIM) started - keeps writes fast");
}

/// Compile only the apps the person opens (plus System UI and the home screen) - compiling every system
/// component would take hours on budget chips. Returns how many were compiled.
fn full_compile(adb: &Adb) -> usize {
    let tmp = std::env::temp_dir().join("clearphone-compile.sh");
    if std::fs::write(&tmp, COMPILE_SCRIPT.replace("\r\n", "\n")).is_err() {
        return 0;
    }
    adb.push(&tmp, COMPILE_REMOTE);
    let mut compiled = 0;
    adb.shell_stream(&format!("sh {COMPILE_REMOTE}"), |line| {
        if let Some(s) = parse_compile_line(line) {
            compiled += usize::from(s.ok);
            let mark = if s.ok { "" } else { " (skipped)" };
            let done = chart::bar(s.index as f64 / s.total.max(1) as f64);
            ui::dim(&format!("{done} {}/{} {}{mark}", s.index, s.total, s.package));
        }
    });
    let _ = adb.sh(&format!("rm -f {COMPILE_REMOTE}"));
    let _ = std::fs::remove_file(&tmp);
    compiled
}

fn enough_space_for_full_compile(adb: &Adb) -> bool {
    diagnostics::storage(adb).is_some_and(|s| {
        s.free_mb >= config::COMPILE_MIN_FREE_MB
            && s.free_mb as f64 >= s.total_mb as f64 * config::COMPILE_MIN_FREE_FRACTION
    })
}

/// Ahead-of-time compile apps so they start and run faster.
pub fn optimize_apps(adb: &Adb, info: &DeviceInfo, diag: &Diag) {
    ui::step("App optimization (compiling)");
    if info.sdk < COMPILE_MIN_SDK {
        ui::warn("Needs Android 7 or newer - skipped");
    } else if diag.battery_level.is_some_and(|l| l < COMPILE_MIN_BATTERY) && !diag.charging {
        ui::warn("Battery below 20% - app optimization skipped (charge the phone and run again)");
    } else if enough_space_for_full_compile(adb) {
        ui::say("Fully compiling every app with an icon - they will start and run faster.");
        ui::dim("Up to an hour on budget phones. Keep the phone connected...");
        ui::ok(&format!("Fully compiled apps: {}", full_compile(adb)));
    } else {
        ui::warn("Not enough free storage for full compilation - running the standard optimization");
        ui::say("Optimizing (compiling) apps - this may take 5-20 minutes...");
        let _ = adb.sh("cmd package bg-dexopt-job");
        ui::ok("Apps optimized");
    }
}

pub fn stop_background(adb: &Adb) {
    let _ = adb.sh("am kill-all");
    ui::ok("Background processes stopped");
}
