//! Undo: bring back removed apps and the original settings from this phone's reports.

use crate::host::paths::Paths;
use crate::host::report;
use crate::phone::brands::Brand;
use crate::phone::packages::Packages;
use crate::phone::Phone;
use crate::removal::lists;
use crate::tune::settings::restore_setting;
use crate::ui;
use std::collections::BTreeMap;

struct Removed {
    method: String,
    desc: String,
}

fn gather_removed(dirs: &[std::path::PathBuf]) -> BTreeMap<String, Removed> {
    let mut map = BTreeMap::new();
    for d in dirs {
        let text = std::fs::read_to_string(d.join("removed.tsv")).unwrap_or_default();
        for line in text.lines() {
            let c: Vec<&str> = line.split('\t').collect();
            if c.len() >= 2 {
                map.insert(
                    c[0].to_string(),
                    Removed { method: c[1].to_string(), desc: c.get(2).copied().unwrap_or("").to_string() },
                );
            }
        }
    }
    map
}

fn restore_apps(phone: &Phone, paths: &Paths, dirs: &[std::path::PathBuf], assume_yes: bool) {
    let adb = &phone.adb;
    let removed = gather_removed(dirs);
    let pkgs = Packages::load(adb);
    let todo: Vec<(&String, &Removed)> = removed
        .iter()
        .filter(|(name, r)| match r.method.as_str() {
            "uninstalled" => !pkgs.contains(name),
            "disabled" => pkgs.get(name).map(|p| !p.enabled).unwrap_or(false),
            _ => false,
        })
        .collect();

    if todo.is_empty() {
        ui::ok("Nothing to restore - every app removed earlier is back");
        return;
    }
    let rows: Vec<(String, String, bool)> = todo.iter().map(|(n, r)| ((*n).clone(), r.desc.clone(), true)).collect();
    ui::show_list(&rows);
    let chosen: Vec<(&String, &Removed)> = if assume_yes {
        todo.clone()
    } else {
        ui::say("Enter - restore all; or numbers to restore only those; 0 - cancel");
        let ans = ui::read_line("Your choice");
        if ans == "0" {
            Vec::new()
        } else if ans.is_empty() {
            todo.clone()
        } else {
            let pick = ui::parse_numbers(&ans, todo.len());
            todo.iter().enumerate().filter(|(i, _)| pick.contains(&(i + 1))).map(|(_, e)| *e).collect()
        }
    };

    for (name, r) in chosen {
        let good = if r.method == "disabled" {
            adb.sh(&format!("pm enable --user 0 {name}")).contains("enabled")
        } else {
            let mut out = adb.sh(&format!("pm install-existing --user 0 {name}"));
            if !out.contains("installed") {
                out = adb.sh(&format!("pm install-existing {name}"));
            }
            out.contains("installed")
        };
        if good {
            ui::ok(name);
            lists::add_keep(&paths.keep_file(&phone.info.device_id), std::slice::from_ref(name));
        } else {
            ui::warn(&format!("{name}: could not restore"));
        }
    }
}

pub fn run(phone: &Phone, paths: &Paths, assume_yes: bool) {
    let (adb, info) = (&phone.adb, &phone.info);
    ui::step("Restoring removed apps");
    let dirs = report::device_reports(paths, &info.device_id);
    restore_apps(phone, paths, &dirs, assume_yes);

    let settings = report::read_first_values(&dirs, "tweaks.tsv", 2);
    if !settings.is_empty()
        && ui::ask_yes_no("Restore the original settings too (animations, DNS, text size, scanning)?", true, assume_yes)
    {
        for (key, val) in &settings {
            if let Some((ns, k)) = key.split_once('\t') {
                restore_setting(adb, ns, k, val);
            }
        }
        ui::ok("Settings restored");
    }

    let bg = report::read_first_values(&dirs, "background.tsv", 1);
    if !bg.is_empty() {
        let names: Vec<&str> = bg.iter().map(|(k, _)| k.as_str()).collect();
        if ui::ask_yes_no(&format!("Allow background activity again ({})?", names.join(", ")), true, assume_yes) {
            for (pkg, mode) in &bg {
                let _ = adb.sh(&format!("cmd appops set {pkg} RUN_ANY_IN_BACKGROUND {mode}"));
            }
            ui::ok("Background activity allowed again");
            if info.brand == Brand::Xiaomi {
                ui::warn("On Xiaomi also set App info -> Battery back to \"Battery saver (recommended)\".");
            }
        }
    }

    let nt = report::read_first_values(&dirs, "notifications.tsv", 1);
    if !nt.is_empty() {
        let names: Vec<&str> = nt.iter().map(|(k, _)| k.as_str()).collect();
        if ui::ask_yes_no(&format!("Allow notifications again ({})?", names.join(", ")), true, assume_yes) {
            for (pkg, was) in &nt {
                let _ = adb.sh(&format!(
                    "pm clear-permission-flags {pkg} android.permission.POST_NOTIFICATIONS user-set user-fixed"
                ));
                if was == "granted" {
                    let _ = adb.sh(&format!("pm grant {pkg} android.permission.POST_NOTIFICATIONS"));
                }
            }
            ui::ok("Notifications can be turned on again (the apps will ask themselves)");
        }
    }
}
