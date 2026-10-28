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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompileState {
    Compiled,
    /// Android had compiled it already; left as it is.
    AlreadyCompiled,
    /// Still not compiled afterwards (some firmware keeps certain apps interpreted).
    Failed,
}

/// One line of compile-apps.sh output: "12/47 OK com.pkg", "12/47 == com.pkg" or "12/47 -- com.pkg".
pub struct CompileStep {
    pub index: usize,
    pub total: usize,
    pub state: CompileState,
    pub package: String,
}

pub fn parse_compile_line(line: &str) -> Option<CompileStep> {
    let mut it = line.split_whitespace();
    let (index, total) = it.next()?.split_once('/')?;
    let state = match it.next()? {
        "OK" => CompileState::Compiled,
        "==" => CompileState::AlreadyCompiled,
        "--" => CompileState::Failed,
        _ => return None,
    };
    Some(CompileStep { index: index.parse().ok()?, total: total.parse().ok()?, state, package: it.next()?.to_string() })
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

/// Compile the apps the person opens (plus System UI and the home screen) that Android has not compiled -
/// compiling every system component would take hours on budget chips. Returns how many were compiled.
fn compile_uncompiled_apps(adb: &Adb) -> usize {
    let tmp = std::env::temp_dir().join("clearphone-compile.sh");
    if std::fs::write(&tmp, COMPILE_SCRIPT.replace("\r\n", "\n")).is_err() {
        return 0;
    }
    adb.push(&tmp, COMPILE_REMOTE);
    let (mut compiled, mut already) = (0, 0);
    adb.shell_stream(&format!("sh {COMPILE_REMOTE}"), |line| {
        if let Some(s) = parse_compile_line(line) {
            let mark = match s.state {
                CompileState::Compiled => {
                    compiled += 1;
                    ""
                }
                CompileState::AlreadyCompiled => {
                    already += 1;
                    " (already compiled by Android)"
                }
                CompileState::Failed => " (Android keeps this one uncompiled)",
            };
            let done = chart::bar(s.index as f64 / s.total.max(1) as f64);
            ui::dim(&format!("{done} {}/{} {}{mark}", s.index, s.total, s.package));
        }
    });
    let _ = adb.sh(&format!("rm -f {COMPILE_REMOTE}"));
    let _ = std::fs::remove_file(&tmp);
    ui::ok(&format!("Compiled now: {compiled}; already compiled by Android and left as they are: {already}"));
    compiled
}

fn enough_space_for_full_compile(adb: &Adb) -> bool {
    diagnostics::storage(adb).is_some_and(|s| {
        s.free_mb >= config::COMPILE_MIN_FREE_MB
            && s.free_mb as f64 >= s.total_mb as f64 * config::COMPILE_MIN_FREE_FRACTION
    })
}

/// Ahead-of-time compile the apps Android has not compiled yet. Returns how many were compiled now
/// (Android's own background job, used when storage is short, counts as one).
pub fn optimize_apps(adb: &Adb, info: &DeviceInfo, diag: &Diag) -> usize {
    ui::step("App optimization (compiling)");
    if info.sdk < COMPILE_MIN_SDK {
        ui::warn("Needs Android 7 or newer - skipped");
        0
    } else if diag.battery_level.is_some_and(|l| l < COMPILE_MIN_BATTERY) && !diag.charging {
        ui::warn("Battery below 20% - app optimization skipped (charge the phone and run again)");
        0
    } else if enough_space_for_full_compile(adb) {
        ui::say("Compiling the apps with an icon that Android has not compiled yet - they will start faster.");
        ui::dim("Apps Android already compiled are left alone. Up to half an hour on budget phones...");
        compile_uncompiled_apps(adb)
    } else {
        ui::warn("Little free storage - running Android's own background optimization instead");
        ui::say("Optimizing (compiling) apps - this may take 5-20 minutes...");
        let _ = adb.sh("cmd package bg-dexopt-job");
        ui::ok("Apps optimized");
        1
    }
}

pub fn stop_background(adb: &Adb) {
    let _ = adb.sh("am kill-all");
    ui::ok("Background processes stopped");
}
