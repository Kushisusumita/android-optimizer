//! Optimize, Maximum and Check: every question first, then an unattended run in five stages.

use super::snapshot::Snapshot;
use super::{questions, steps, Session};
use crate::config::{self, Action};
use crate::host::report;
use crate::inspect::score;
use crate::removal::candidates::Candidate;
use crate::removal::lists::Section;
use crate::tune::settings::{Access, Tweaks};
use crate::tune::{self, adblock, apps, cleanup, Change, Plan};
use crate::ui::{self, chart::Progress};

pub fn run(action: Action, s: &Session, before: &Snapshot) {
    let shown: Vec<&Candidate> =
        before.cands.items.iter().filter(|c| action == Action::Max || c.section == Section::Safe).collect();
    show_new_apps(s, before);
    s.report.write("skipped.txt", &before.cands.notes.join("\n"));
    if action == Action::Check {
        preview(&shown, &before.cands.notes);
        return;
    }
    // Every question first; after this the run needs nobody at the keyboard.
    let chosen = questions::select_removals(s, &shown);
    let access = questions::settings_access(s);
    let plan = questions::choose_plan(s, &before.pkgs, access.as_ref());
    let launch = steps::measure_speed(s);

    let mut progress = Progress::new(5);
    progress.next("Removing ads and junk");
    let removed = apps::remove_packages(&s.phone.adb, &s.report, &chosen);
    show_interim(before, removed);
    progress.next("Settings, ad blocking and limits");
    match &access {
        Some(access) => apply_settings(s, &plan, access),
        None => ui::dim("Skipped: the phone does not allow changing settings"),
    }
    progress.next("Cleaning caches");
    if plan.wants(Change::CleanCaches) {
        cleanup::clean_caches(&s.phone.adb);
    } else {
        ui::dim("Skipped (not chosen)");
    }
    progress.next("Compiling apps");
    steps::compile(s, &before.diag, (!plan.wants(Change::Compile)).then_some("not chosen"));
    progress.next("Measuring the result");
    steps::finish(s, before, launch);
}

fn show_new_apps(s: &Session, before: &Snapshot) {
    let fresh = report::new_since_last_run(&s.paths, &s.phone.info.device_id, &before.pkgs);
    if !fresh.is_empty() {
        ui::step("Installed since the last run");
        for n in &fresh {
            ui::say(n);
        }
    }
}

fn preview(shown: &[&Candidate], notes: &[String]) {
    ui::step(&format!("Would remove {} apps (Check changes nothing)", shown.len()));
    ui::show_list(&questions::rows(shown));
    if !notes.is_empty() {
        ui::dim("Left alone:");
        for n in notes {
            ui::dim(n);
        }
    }
}

fn show_interim(before: &Snapshot, removed: usize) {
    let junk = before.cands.junk_count();
    let was = score::score(&before.diag, junk);
    let now = score::score(&before.diag, junk.saturating_sub(removed));
    ui::say(&format!("Health score so far: {was} -> {now}"));
}

fn apply_settings(s: &Session, plan: &Plan, access: &Access) {
    let mut tw = Tweaks::new(&s.phone.adb, &s.report, access);
    tune::apply(&mut tw, &s.phone.info, plan, &s.paths);
    if plan.wants(Change::AdBlockDns) {
        adblock::enable(&mut tw, &s.phone.info, s.opts.dns_host.as_deref().unwrap_or(config::DEFAULT_DNS_HOST));
    }
    if tw.secure_denied {
        ui::warn("The phone refused some changes:");
        for line in s.phone.info.brand.setup_help() {
            ui::dim(line);
        }
    }
}
