//! Steps shared by the optimize and speed-up runs: the app start measurement, compilation, and the
//! "after" measurement with the chart and the restart question.

use super::snapshot::{self, Snapshot};
use super::Session;
use crate::inspect::diagnostics::Diag;
use crate::inspect::hardware;
use crate::tune::cleanup;
use crate::ui::{self, chart};

pub fn measure_speed(s: &Session) -> Option<i64> {
    ui::step("Measuring app start speed (Settings will open and close a few times)");
    let ms = hardware::launch_ms(&s.phone.adb);
    if let Some(ms) = ms {
        ui::say(&format!("Settings opens in {ms} ms"));
    }
    ms
}

/// Compile apps unless there is a reason to skip it, then stop leftover background processes.
pub fn compile(s: &Session, diag: &Diag, skip_reason: Option<&str>) {
    match skip_reason {
        Some(reason) => ui::dim(&format!("Skipped ({reason})")),
        None => cleanup::optimize_apps(&s.phone.adb, &s.phone.info, diag),
    }
    cleanup::stop_background(&s.phone.adb);
}

/// Measure again, draw before/after, point to the report, offer a restart.
pub fn finish(s: &Session, before: &Snapshot, launch_before: Option<i64>) {
    let after = snapshot::take(s);
    s.report.write_packages("packages-after.txt", &after.pkgs);
    let launch_after = launch_before.and_then(|_| hardware::launch_ms(&s.phone.adb));
    chart::show_chart(&snapshot::measure(before, launch_before), &snapshot::measure(&after, launch_after));
    ui::dim(&format!("Report (used by Restore): {}", s.report.path().display()));
    maybe_reboot(s, &after.diag);
}

fn maybe_reboot(s: &Session, diag: &Diag) {
    if diag.stuck_volume_key.is_some() {
        if s.opts.reboot {
            ui::warn("Not restarting: a volume key looks stuck, and the phone would boot into recovery.");
        }
        return;
    }
    if s.opts.reboot || ui::ask_yes_no("Restart the phone now to finish (recommended)?", false, s.opts.yes) {
        s.phone.adb.reboot();
        ui::ok("Restarting the phone");
    }
}
