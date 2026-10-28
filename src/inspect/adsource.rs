//! Find the app behind constant pop-up ads. Read-only: it points at likely sources, changes nothing.

use super::perms::{self, PowerApps};
use crate::phone::adb::Adb;
use crate::phone::packages::Packages;
use crate::ui;

/// Recently installed apps worth suspecting first.
const RECENT_APPS_SHOWN: usize = 8;

/// The package of the first "com.pkg.name/Activity" token anywhere in the text.
pub fn first_component_package(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '{' || c == '}')
        .filter_map(|tok| tok.split_once('/').map(|(pkg, _)| pkg))
        .find(|pkg| pkg.contains('.') && pkg.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_'))
        .map(String::from)
}

/// The install date (YYYY-MM-DD) from `dumpsys package <pkg>` output.
pub fn install_date(dump: &str) -> Option<String> {
    let date: String = dump.split("firstInstallTime=").nth(1)?.chars().take(10).collect();
    (date.len() == 10).then_some(date)
}

pub fn find(adb: &Adb, pkgs: &Packages) {
    ui::step("Finding the source of pop-up ads");
    show_focused_app(adb, pkgs);
    let apps = perms::power_apps(adb, pkgs);
    show_overlay_apps(&apps);
    show_powerful_apps(&apps);
    show_recent_apps(adb, pkgs);
    show_other_sources(adb);
}

/// Run this WHILE an ad is showing: the app on screen is then the culprit.
fn show_focused_app(adb: &Adb, pkgs: &Packages) {
    let focus = ["dumpsys activity activities | grep -m1 ResumedActivity", "dumpsys window | grep -m1 mCurrentFocus"]
        .iter()
        .find_map(|cmd| first_component_package(&adb.sh(cmd)));
    if let Some(focus) = focus {
        let tag = if pkgs.is_third_party(&focus) {
            " <- a third-party app; if an ad is on screen now, this is likely it"
        } else {
            ""
        };
        ui::say(&format!("App on screen now: {focus}{tag}"));
    }
    ui::dim("TIP: when an ad pops up, do NOT close it - run this action right then to catch the app above.");
}

fn show_overlay_apps(apps: &PowerApps) {
    ui::step(&format!("Apps that can show windows over everything (the usual ad source): {}", apps.overlay.len()));
    if apps.overlay.is_empty() {
        ui::ok("No third-party app can draw over other apps");
        return;
    }
    for o in &apps.overlay {
        ui::warn(o);
    }
    ui::dim("Turn this off: Settings -> Apps -> Special app access -> Display over other apps.");
}

fn show_powerful_apps(apps: &PowerApps) {
    let rows: Vec<String> = apps
        .accessibility
        .iter()
        .map(|p| format!("{p} (Accessibility - can read the screen and tap for you)"))
        .chain(apps.notification_listeners.iter().map(|p| format!("{p} (reads all your notifications)")))
        .chain(apps.installers.iter().map(|p| format!("{p} (can install other apps)")))
        .collect();
    if rows.is_empty() {
        return;
    }
    ui::step(&format!("Apps with powerful permissions adware abuses: {}", rows.len()));
    for r in &rows {
        ui::warn(r);
    }
}

fn show_recent_apps(adb: &Adb, pkgs: &Packages) {
    let mut recent: Vec<(String, &str)> = pkgs
        .values()
        .filter(|p| !p.system && p.enabled)
        .filter_map(|p| {
            install_date(&adb.sh(&format!("dumpsys package {} | grep -m1 firstInstallTime", p.name)))
                .map(|d| (d, p.name.as_str()))
        })
        .collect();
    recent.sort_by(|a, b| b.cmp(a));
    if recent.is_empty() {
        return;
    }
    ui::step("Most recently installed apps (ads that just started often come from the newest):");
    for (d, n) in recent.iter().take(RECENT_APPS_SHOWN) {
        ui::say(&format!("{d}  {n}"));
    }
}

fn show_other_sources(adb: &Adb) {
    ui::step("Other common sources of pop-ups");
    let browser = first_component_package(
        &adb.sh("cmd package resolve-activity --brief -a android.intent.action.VIEW -d http://example.com"),
    )
    .unwrap_or_else(|| "your browser".into());
    ui::say(&format!("- Browser notifications: open {browser} -> Settings -> Notifications / Site settings and turn off sites you did not subscribe to."));
    ui::say("- Calendar spam: if you get ad \"events\", open Calendar, delete them and stop adding events from unknown sources.");
    ui::say("- To remove a bad app: hold its icon -> Uninstall, or run the Optimize/Security action.");
    ui::dim("This only finds likely sources - it changes nothing. Review the apps above and remove the culprit.");
}
