//! Every question an optimization run asks, in one place. With --yes each one takes its safe default.

use super::Session;
use crate::config::{Opts, TextSize};
use crate::inspect::leftovers;
use crate::phone::info::DeviceInfo;
use crate::phone::packages::Packages;
use crate::removal::candidates::Candidate;
use crate::removal::lists;
use crate::tune::settings::Access;
use crate::tune::{Change, Plan};
use crate::ui;

/// How many times to re-check after the person says the switch is on.
const ACCESS_RETRIES: usize = 5;

pub fn rows(items: &[&Candidate]) -> Vec<(String, String, bool)> {
    items.iter().map(|c| (c.name.clone(), c.desc.clone(), c.enabled)).collect()
}

/// The removals the person agreed to, as (package, description). Kept apps are remembered for this phone.
pub fn select_removals(s: &Session, shown: &[&Candidate]) -> Vec<(String, String)> {
    if shown.is_empty() {
        ui::ok("No ads or junk found");
        return Vec::new();
    }
    ui::step(&format!("Ads and junk to remove: {}", shown.len()));
    ui::show_list(&rows(shown));
    let answer = if s.opts.yes {
        String::new()
    } else {
        ui::say("Enter - remove all; numbers (e.g. 2,5) - keep those apps (not offered again); 0 - remove nothing");
        ui::read_line("Your choice")
    };
    if answer == "0" {
        return Vec::new();
    }
    let (kept, removed) = ui::split_by_numbers(shown, &ui::parse_numbers(&answer, shown.len()));
    let kept: Vec<String> = kept.iter().map(|c| c.name.clone()).collect();
    lists::add_keep(&s.paths.keep_file(&s.phone.info.device_id), &kept);
    removed.iter().map(|c| (c.name.clone(), c.desc.clone())).collect()
}

/// Make sure settings can be changed; on brands that lock them, show the brand's exact switch and wait.
pub fn settings_access(s: &Session) -> Option<Access> {
    let adb = &s.phone.adb;
    if let Some(access) = Access::check(adb) {
        return Some(access);
    }
    ui::warn("The phone does not allow changing settings - ad blocking, text size and animations cannot be applied.");
    for line in s.phone.info.brand.setup_help() {
        ui::dim(line);
    }
    if s.opts.yes {
        return None;
    }
    for _ in 0..ACCESS_RETRIES {
        let a = ui::read_line("Turn it on and press Enter (n = continue without settings)").to_ascii_lowercase();
        if matches!(a.as_str(), "n" | "no") {
            return None;
        }
        if let Some(access) = Access::check(adb) {
            ui::ok("Settings access granted");
            return Some(access);
        }
        ui::warn("Not yet - check that the switch stayed on");
    }
    None
}

/// The automatic changes worth offering on this phone: they apply to it, the phone lets us make them, and
/// the command line did not rule them out.
pub fn offered_changes(info: &DeviceInfo, opts: &Opts, settings_allowed: bool, leftovers_found: bool) -> Vec<Change> {
    let ruled_out = |c: &Change| match c {
        Change::AdBlockDns => opts.no_dns,
        Change::Compile => opts.no_compile,
        Change::RepairSettings => !leftovers_found,
        _ => false,
    };
    Change::ALL
        .into_iter()
        .filter(|c| c.applies_to(info) && (settings_allowed || !c.needs_settings_access()) && !ruled_out(c))
        .collect()
}

/// Everything the run will do on its own, as one checklist: Enter keeps all, numbers skip those.
fn confirm_changes(s: &Session, offered: Vec<Change>) -> Vec<Change> {
    if s.opts.yes || offered.is_empty() {
        return offered;
    }
    ui::step("The run will also do this (all reversible)");
    let rows: Vec<(String, String, bool)> =
        offered.iter().map(|c| (c.title().to_string(), String::new(), true)).collect();
    ui::show_list(&rows);
    let answer = ui::read_line("Enter - do all of it; numbers (e.g. 1,6) - skip those");
    ui::split_by_numbers(&offered, &ui::parse_numbers(&answer, offered.len())).1
}

/// The person's choices for this run. Without settings access only cleanup and compilation remain.
pub fn choose_plan(s: &Session, pkgs: &Packages, access: Option<&Access>) -> Plan {
    let leftovers_found = !leftovers::on_phone(&s.phone.adb).is_empty();
    let changes = confirm_changes(s, offered_changes(&s.phone.info, &s.opts, access.is_some(), leftovers_found));
    if access.is_none() {
        return Plan { changes, ..Plan::default() };
    }
    let (font, display) = text_scales(s);
    let background_question = "Limit background activity (saves battery and RAM) for which apps?";
    Plan {
        font,
        display,
        background: choose_apps(s, pkgs, &s.opts.background, background_question),
        mute: choose_apps(s, pkgs, &s.opts.mute, "Turn off notifications for which apps?"),
        changes,
    }
}

fn text_scales(s: &Session) -> (f64, f64) {
    let explicit = s.opts.font_scale.is_some() || s.opts.display_scale.is_some();
    let preset = match s.opts.text_size {
        Some(t) => t,
        None if s.opts.yes || explicit => TextSize::Unchanged,
        None => ask_text_size(),
    };
    let (font, display) = preset.scales();
    (s.opts.font_scale.unwrap_or(font), s.opts.display_scale.unwrap_or(display))
}

fn ask_text_size() -> TextSize {
    ui::step("Bigger text? (helps with poor eyesight)");
    ui::say("0 - leave as is   1 - large   2 - larger   3 - largest");
    TextSize::parse(&ui::read_line("Your choice (Enter = 0)")).unwrap_or(TextSize::Unchanged)
}

/// Apps from lists/background.txt that are installed, offered as a numbered list. Names given on the
/// command line are checked here, once: only installed ones go on.
fn choose_apps(s: &Session, pkgs: &Packages, preset: &[String], question: &str) -> Vec<String> {
    if !preset.is_empty() {
        return preset.iter().filter(|p| pkgs.contains(p)).cloned().collect();
    }
    if s.opts.yes {
        return Vec::new();
    }
    let offer: Vec<(String, String, bool)> = lists::read_list(&s.paths.list_file("background"))
        .iter()
        .flat_map(|spec| pkgs.matching(&spec.pattern).into_iter().map(|n| (n, spec.desc.clone(), true)))
        .collect();
    if offer.is_empty() {
        return Vec::new();
    }
    ui::step(question);
    ui::show_list(&offer);
    let numbers = ui::parse_numbers(&ui::read_line("Numbers, e.g. 1,3 (Enter = none)"), offer.len());
    ui::split_by_numbers(&offer, &numbers).0.into_iter().map(|(n, _, _)| n).collect()
}
