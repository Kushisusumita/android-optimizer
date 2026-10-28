//! Every question an optimization run asks, in one place. With --yes each one takes its safe default.

use super::Session;
use crate::config::TextSize;
use crate::phone::packages::Packages;
use crate::removal::candidates::Candidate;
use crate::removal::lists;
use crate::tune::settings::test_writable;
use crate::tune::Plan;
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
pub fn settings_access(s: &Session) -> bool {
    let adb = &s.phone.adb;
    if test_writable(adb) {
        return true;
    }
    ui::warn("The phone does not allow changing settings - ad blocking, text size and animations cannot be applied.");
    for line in s.phone.info.brand.setup_help() {
        ui::dim(line);
    }
    if s.opts.yes {
        return false;
    }
    for _ in 0..ACCESS_RETRIES {
        let a = ui::read_line("Turn it on and press Enter (n = continue without settings)").to_ascii_lowercase();
        if matches!(a.as_str(), "n" | "no") {
            return false;
        }
        if test_writable(adb) {
            ui::ok("Settings access granted");
            return true;
        }
        ui::warn("Not yet - check that the switch stayed on");
    }
    false
}

pub fn choose_plan(s: &Session, pkgs: &Packages) -> Plan {
    let (font, display) = text_scales(s);
    let background_question = "Limit background activity (saves battery and RAM) for which apps?";
    Plan {
        font,
        display,
        background: choose_apps(s, pkgs, &s.opts.background, background_question),
        mute: choose_apps(s, pkgs, &s.opts.mute, "Turn off notifications for which apps?"),
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
