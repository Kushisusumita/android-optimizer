//! Steps shared by the optimize and speed-up runs: measuring speed before the changes, compilation, and
//! the fair "after" - once the phone has cooled down - with the results in words and the restart question.

use super::results::{self, RunFacts};
use super::snapshot::{self, Snapshot, Speed};
use super::Session;
use crate::inspect::compiled;
use crate::inspect::diagnostics::Diag;
use crate::inspect::speed::{self, Probe};
use crate::tune::cleanup;
use crate::ui::{self, chart};

/// What "before" was, kept so "after" measures exactly the same apps.
pub struct Baseline {
    probes: Vec<Probe>,
    speed: Speed,
}

fn measure_speed(s: &Session, probes: &[Probe]) -> Speed {
    let adb = &s.phone.adb;
    let launches = probes
        .iter()
        .filter_map(|p| {
            let ms = speed::launch_ms(adb, p)?;
            ui::say(&format!("{} opens in {ms} ms", p.name));
            Some((p.name.to_string(), ms))
        })
        .collect();
    let compiled_icon_apps = compiled::icon_apps(adb);
    ui::say(&format!("Apps with an icon compiled: {} of {}", compiled_icon_apps.0, compiled_icon_apps.1));
    Speed { launches, compiled_icon_apps }
}

pub fn baseline(s: &Session) -> Baseline {
    ui::step("Measuring how fast apps start (a few system apps open and close - please do not touch the phone)");
    let probes = speed::probes(&s.phone.adb);
    let speed = measure_speed(s, &probes);
    Baseline { probes, speed }
}

/// Compile apps unless there is a reason to skip it, then stop leftover background processes.
/// Returns how many apps were compiled.
pub fn compile(s: &Session, diag: &Diag, skip_reason: Option<&str>) -> usize {
    let compiled = match skip_reason {
        Some(reason) => {
            ui::dim(&format!("Skipped ({reason})"));
            0
        }
        None => cleanup::optimize_apps(&s.phone.adb, &s.phone.info, diag),
    };
    cleanup::stop_background(&s.phone.adb);
    compiled
}

/// Let the phone cool down, measure the same things again, explain every change, offer a restart.
pub fn finish(s: &Session, before: &Snapshot, base: &Baseline, facts: &RunFacts) {
    ui::step("Waiting for the phone to cool down, so the comparison is fair");
    if !speed::wait_until_cool(&s.phone.adb) {
        ui::dim("Still warm or busy after 3 minutes - measuring anyway; it may look slower than it is.");
    }
    let after = snapshot::take(s);
    s.report.write_packages("packages-after.txt", &after.pkgs);
    let after_speed = measure_speed(s, &base.probes);
    let rows = results::rows(&snapshot::measure(before, &base.speed), &snapshot::measure(&after, &after_speed), facts);
    chart::show_results(&rows);
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
