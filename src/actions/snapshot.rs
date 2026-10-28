//! The phone's state at one moment - packages, diagnostics and what could still be removed - and how it
//! is shown and condensed for the before/after chart.

use super::Session;
use crate::inspect::diagnostics::{self, Diag};
use crate::inspect::{hardware, leftovers, score};
use crate::phone::packages::Packages;
use crate::removal::candidates::{self, Candidates, Overrides};
use crate::removal::lists;
use crate::ui::{self, chart::Measure};
use std::collections::HashSet;

pub struct Snapshot {
    pub pkgs: Packages,
    pub diag: Diag,
    pub cands: Candidates,
}

/// How fast and how compiled the apps are at one moment (measured only by the runs that change it).
pub struct Speed {
    pub launches: Vec<(String, i64)>,
    pub compiled_icon_apps: (usize, usize),
}

pub fn find_candidates(s: &Session, pkgs: &Packages) -> Candidates {
    let keep: HashSet<String> = lists::read_patterns(&s.paths.keep_file(&s.phone.info.device_id)).into_iter().collect();
    let exclude: HashSet<String> = s.opts.exclude.iter().cloned().collect();
    let overrides = Overrides { include: &s.opts.include, exclude: &exclude, keep: &keep };
    candidates::candidates(&s.paths, &s.phone.info, pkgs, &overrides)
}

pub fn take(s: &Session) -> Snapshot {
    let pkgs = Packages::load(&s.phone.adb);
    let diag = diagnostics::collect(&s.phone, &pkgs);
    let cands = find_candidates(s, &pkgs);
    Snapshot { pkgs, diag, cands }
}

pub fn show(s: &Session, snap: &Snapshot) {
    ui::step("Phone health");
    diagnostics::show(&snap.diag);
    diagnostics::warnings(&s.phone.adb, &snap.diag, &s.phone.info);
    for l in leftovers::on_phone(&s.phone.adb) {
        ui::warn(&format!("A setting slows the phone down: {} (Optimize puts it back)", l.what));
    }
    hardware::show_specs(&s.phone, &snap.diag);
    hardware::show_load(&s.phone, &snap.diag);
    let points = score::score(&snap.diag, snap.cands.junk_count());
    ui::step(&format!("Health score: {points}/100 ({})", score::label(points)));
}

/// The snapshot condensed for the results, with the speed measured at the same moment.
pub fn measure(snap: &Snapshot, speed: &Speed) -> Measure {
    let points = score::score(&snap.diag, snap.cands.junk_count());
    let d = &snap.diag;
    Measure {
        score: points,
        score_label: score::label(points),
        launches: speed.launches.clone(),
        compiled_icon_apps: speed.compiled_icon_apps,
        ram_free_mb: d.ram_free_mb,
        ram_total_mb: d.ram_total_mb,
        storage_free_mb: d.storage.as_ref().map(|s| s.free_mb).unwrap_or(0),
        storage_total_mb: d.storage.as_ref().map(|s| s.total_mb).unwrap_or(0),
        packages: d.packages,
    }
}
