//! Changes to apps: removing them for the current user, limiting background work, muting notifications.

use super::screen_flows;
use super::settings::Tweaks;
use super::Plan;
use crate::host::paths::Paths;
use crate::host::report::Report;
use crate::phone::adb::Adb;
use crate::phone::brands::Brand;
use crate::phone::info::DeviceInfo;
use crate::phone::packages;
use crate::removal::lists;
use crate::ui;

pub const BACKGROUND_OP: &str = "RUN_ANY_IN_BACKGROUND";
/// The older background switch that apps built for Android 7 or older obey.
pub const LEGACY_BACKGROUND_OP: &str = "RUN_IN_BACKGROUND";
/// Android 8 (API 26) introduced the newer switch.
const LEGACY_TARGET_SDK: i32 = 26;
const NOTIFICATION_PERMISSION: &str = "android.permission.POST_NOTIFICATIONS";
/// Android 13 made notifications a runtime permission.
const NOTIFICATION_PERMISSION_SDK: i32 = 33;

/// The mode in `cmd appops get <pkg> <op>` output ("RUN_ANY_IN_BACKGROUND: ignore; time=..." -> "ignore").
pub fn parse_appops_mode(output: &str, op: &str) -> String {
    output
        .split(&format!("{op}:"))
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .map(|m| m.trim_end_matches(';').to_string())
        .unwrap_or_else(|| "default".into())
}

/// Remove for the current user only (the APK stays, so Restore can bring it back); disable if that fails.
/// Returns how many were removed.
pub fn remove_packages(adb: &Adb, report: &Report, items: &[(String, String)]) -> usize {
    let mut count = 0;
    for (name, desc) in items {
        let r = adb.sh(&format!("pm uninstall -k --user 0 {name}"));
        let method = if r.contains("Success") {
            Some("uninstalled")
        } else if adb.sh(&format!("pm disable-user --user 0 {name}")).contains("disabled") {
            Some("disabled")
        } else {
            None
        };
        match method {
            Some(m) => {
                report.append("removed.tsv", &format!("{name}\t{m}\t{desc}"));
                count += 1;
                ui::ok(&if m == "disabled" { format!("{name} (disabled)") } else { name.clone() });
            }
            None => ui::warn(&format!("{name}: {}", r.trim())),
        }
    }
    count
}

/// Is the package on the battery-optimization allowlist because someone added it ("user,<pkg>,<uid>" in
/// `cmd deviceidle whitelist`)? System entries belong to the maker and stay.
pub fn in_user_allowlist(list: &str, pkg: &str) -> bool {
    list.lines().any(|l| {
        let mut fields = l.trim().split(',');
        fields.next() == Some("user") && fields.next() == Some(pkg)
    })
}

impl Tweaks<'_> {
    /// Android's own "Restricted" battery mode, as its Settings app sets it: no background work, off the
    /// battery allowlist, and the older switch too for apps built for Android 7 or older. Every previous
    /// state is recorded for Restore.
    pub fn set_background_restricted(&mut self, pkg: &str) -> bool {
        let ok = self.deny_op(pkg, BACKGROUND_OP, "background.tsv");
        if ok {
            self.leave_battery_allowlist(pkg);
            if packages::target_sdk(self.adb, pkg).is_some_and(|t| t < LEGACY_TARGET_SDK) {
                self.deny_op(pkg, LEGACY_BACKGROUND_OP, "background-legacy.tsv");
            }
        }
        ok
    }

    /// Set an app op to `ignore`; the old mode goes to `file` as "pkg<TAB>old".
    fn deny_op(&mut self, pkg: &str, op: &str, file: &str) -> bool {
        let old = parse_appops_mode(&self.adb.sh(&format!("cmd appops get {pkg} {op}")), op);
        if old == "ignore" {
            return true;
        }
        let ok = self.change(&format!("cmd appops set {pkg} {op} ignore"));
        if ok {
            self.report.append(file, &format!("{pkg}\t{old}"));
        }
        ok
    }

    fn leave_battery_allowlist(&mut self, pkg: &str) {
        let listed = in_user_allowlist(&self.adb.sh("cmd deviceidle whitelist"), pkg);
        if listed && self.change(&format!("cmd deviceidle whitelist -{pkg}")) {
            self.report.append("battery-allowlist.tsv", pkg);
        }
    }

    /// Turn notifications off and stop the app from asking again (Android 13+).
    pub fn set_notifications_blocked(&mut self, pkg: &str, sdk: i32) -> bool {
        if sdk < NOTIFICATION_PERMISSION_SDK {
            return false;
        }
        let granted = self.adb.sh(&format!("dumpsys package {pkg}")).contains("POST_NOTIFICATIONS: granted=true");
        let _ = self.adb.sh(&format!("pm revoke {pkg} {NOTIFICATION_PERMISSION}"));
        let ok = self.change(&format!("pm set-permission-flags {pkg} {NOTIFICATION_PERMISSION} user-set user-fixed"));
        if ok {
            self.report.append("notifications.tsv", &format!("{pkg}\t{}", if granted { "granted" } else { "denied" }));
        }
        ok
    }
}

fn media_packages(paths: &Paths) -> Vec<String> {
    lists::read_list(&paths.list_file("background")).into_iter().filter(|s| s.media).map(|s| s.pattern).collect()
}

pub(super) fn apply_background(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan, paths: &Paths) {
    let limited: Vec<String> = plan.background.iter().filter(|p| tw.set_background_restricted(p)).cloned().collect();
    if limited.is_empty() {
        return;
    }
    ui::ok(&format!("Background activity limited: {}", limited.join(", ")));
    if info.brand == Brand::Xiaomi {
        apply_xiaomi_battery_mode(tw.adb, &limited, &media_packages(paths));
    }
}

/// Xiaomi resets the standard limit on every boot, so it is also set in Xiaomi's own battery screen.
fn apply_xiaomi_battery_mode(adb: &Adb, limited: &[String], media: &[String]) {
    let (mut ok, mut fail) = (Vec::new(), Vec::new());
    ui::dim("Xiaomi: setting the limit in Xiaomi's own settings (do not touch the phone for ~1 minute)...");
    for p in limited.iter().filter(|p| !media.contains(p)) {
        if screen_flows::set_xiaomi_background_limit(adb, p) {
            ok.push(p.clone());
        } else {
            fail.push(p.clone());
        }
    }
    let _ = adb.sh("input keyevent KEYCODE_HOME");
    if !ok.is_empty() {
        ui::ok(&format!("Xiaomi: \"close after 10 minutes in the background\" - {} (survives reboots)", ok.join(", ")));
    }
    if !fail.is_empty() {
        ui::warn(&format!("Xiaomi resets the background limit on reboot. Set it by hand for {}:", fail.join(", ")));
        ui::dim("App info -> Battery -> \"Close apps after 10 minutes of background activity\".");
    }
}

pub(super) fn apply_mute(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan) {
    let muted: Vec<String> = plan.mute.iter().filter(|p| tw.set_notifications_blocked(p, info.sdk)).cloned().collect();
    if !muted.is_empty() {
        ui::ok(&format!("Notifications off (and the apps cannot ask again): {}", muted.join(", ")));
    }
}
