//! Changing settings (reversibly), blocking ads via DNS, limiting apps, and the cache/compile cleanup.

use crate::adb::Adb;
use crate::config;
use crate::device::{parse_wm_size, DeviceInfo};
use crate::diagnostics::{self, num_after, Diag};
use crate::lists;
use crate::paths::Paths;
use crate::report::Report;
use crate::screen;
use crate::ui;
use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;

const BACKGROUND_OP: &str = "RUN_ANY_IN_BACKGROUND";
const NOTIFICATION_PERMISSION: &str = "android.permission.POST_NOTIFICATIONS";
/// Android 13 made notifications a runtime permission.
const NOTIFICATION_PERMISSION_SDK: i32 = 33;
/// Used when `wm size` cannot be read: a typical phone's short side.
const DEFAULT_SMALLEST_SIDE_PX: i64 = 1080;

/// The settings the person chose beyond the always-on optimizations. A scale of 0.0 means "leave as is".
pub struct Plan {
    pub font: f64,
    pub display: f64,
    pub background: Vec<String>,
    pub mute: Vec<String>,
}

/// Did the phone refuse a command? (Locked settings answer with a SecurityException or a denial.)
pub fn refused(output: &str) -> bool {
    ["Exception", "denial", "denied", "Error"].iter().any(|w| output.contains(w))
}

/// The density (dpi) for a display-size factor: a multiple of 4, but never so large that the screen
/// becomes narrower than MIN_SMALLEST_WIDTH_DP.
pub fn density_target(physical_dpi: i64, scale: f64, smallest_side_px: i64) -> i64 {
    let wanted = ((physical_dpi as f64 * scale / 4.0).round() * 4.0) as i64;
    let max = (smallest_side_px as f64 * 160.0 / config::MIN_SMALLEST_WIDTH_DP).floor() as i64;
    wanted.min(max)
}

/// The mode in `cmd appops get <pkg> <op>` output ("RUN_ANY_IN_BACKGROUND: ignore; time=..." -> "ignore").
pub fn parse_appops_mode(output: &str, op: &str) -> String {
    output
        .split(&format!("{op}:"))
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .map(|m| m.trim_end_matches(';').to_string())
        .unwrap_or_else(|| "default".into())
}

/// Reversible settings changes. Every original value is recorded once in the report for Restore.
pub struct Tweaks<'a> {
    adb: &'a Adb,
    report: &'a Report,
    saved: HashSet<String>,
    pub secure_denied: bool,
}

impl<'a> Tweaks<'a> {
    pub fn new(adb: &'a Adb, report: &'a Report) -> Self {
        Tweaks { adb, report, saved: HashSet::new(), secure_denied: false }
    }

    fn save_original(&mut self, ns: &str, key: &str, old: &str) {
        if self.saved.insert(format!("{ns}/{key}")) {
            self.report.append("tweaks.tsv", &format!("{ns}	{key}	{old}"));
        }
    }

    /// Run a changing command; false (and the phone marked as locked) if it was refused.
    fn change(&mut self, cmd: &str) -> bool {
        let ok = !refused(&self.adb.sh(cmd));
        self.secure_denied |= !ok;
        ok
    }

    /// Set a value if it differs, remembering the original for Restore.
    pub fn set(&mut self, ns: &str, key: &str, value: &str) -> bool {
        let old = self.adb.get_setting(ns, key);
        if old == value {
            return true;
        }
        let ok = self.change(&format!("settings put {ns} {key} {value}"));
        if ok {
            self.save_original(ns, key, &old);
        }
        ok
    }

    /// Display size ("density"): scales text, buttons and icons together.
    pub fn set_display_scale(&mut self, scale: f64) -> bool {
        let out = self.adb.sh("wm density");
        let Some(physical) = num_after(&out, "Physical density:").filter(|p| *p > 0) else { return false };
        let current = num_after(&out, "Override density:");
        let side = parse_wm_size(&self.adb.sh("wm size")).map(|(w, h)| w.min(h)).unwrap_or(DEFAULT_SMALLEST_SIDE_PX);
        let target = density_target(physical, scale, side);
        if current.unwrap_or(physical) == target {
            return true;
        }
        let ok = self.change(&format!("wm density {target}"));
        if ok {
            self.save_original("wm", "density", &current.map(|c| c.to_string()).unwrap_or_else(|| "reset".into()));
        }
        ok
    }

    /// Stop an app from running in the background; the previous mode is recorded for Restore.
    pub fn set_background_restricted(&mut self, pkg: &str) -> bool {
        let old = parse_appops_mode(&self.adb.sh(&format!("cmd appops get {pkg} {BACKGROUND_OP}")), BACKGROUND_OP);
        if old == "ignore" {
            return true;
        }
        let ok = self.change(&format!("cmd appops set {pkg} {BACKGROUND_OP} ignore"));
        if ok {
            self.report.append("background.tsv", &format!("{pkg}	{old}"));
        }
        ok
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
            self.report.append("notifications.tsv", &format!("{pkg}	{}", if granted { "granted" } else { "denied" }));
        }
        ok
    }
}

/// Can we change settings at all? (Xiaomi blocks this without "USB debugging (Security settings)".)
pub fn test_writable(adb: &Adb) -> bool {
    if refused(&adb.sh("settings put global clearphone_probe 1")) {
        return false;
    }
    let _ = adb.sh("settings delete global clearphone_probe");
    true
}

/// Make sure settings can be changed; on brands that lock them, show the brand's exact switch and wait.
pub fn request_settings_access(adb: &Adb, info: &DeviceInfo, assume_yes: bool) -> bool {
    if test_writable(adb) {
        return true;
    }
    ui::warn("The phone does not allow changing settings - ad blocking, text size and animations cannot be applied.");
    for line in info.brand.setup_help() {
        ui::dim(line);
    }
    if assume_yes {
        return false;
    }
    for _ in 0..5 {
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

pub fn restore_setting(adb: &Adb, ns: &str, key: &str, value: &str) {
    if ns == "wm" {
        if value.is_empty() || value == "reset" {
            let _ = adb.sh(&format!("wm {key} reset"));
        } else {
            let _ = adb.sh(&format!("wm {key} {value}"));
        }
    } else if value.is_empty() || value == "null" {
        let _ = adb.sh(&format!("settings delete {ns} {key}"));
    } else {
        let _ = adb.sh(&format!("settings put {ns} {key} {value}"));
    }
}

fn resolves(adb: &Adb) -> bool {
    for h in ["google.com", "ya.ru"] {
        if adb.sh(&format!("ping -c 1 -W 2 {h}")).contains("PING ") {
            return true;
        }
    }
    false
}

/// System-wide ad/tracker blocking via Private DNS (strict). Verified, and rolled back if it breaks the net.
pub fn enable_adblock_dns(tw: &mut Tweaks, info: &DeviceInfo, dns_host: &str) {
    ui::step("Ad and tracker blocking in all apps and browsers (Private DNS)");
    if info.sdk < 28 {
        ui::warn("Needs Android 9 or newer - skipped");
        return;
    }
    let mode = tw.adb.get_setting("global", "private_dns_mode");
    let spec = tw.adb.get_setting("global", "private_dns_specifier");
    if mode == "hostname" && spec == dns_host {
        ui::ok(&format!("Already on ({dns_host})"));
        return;
    }
    if mode == "hostname" && !spec.is_empty() && spec != "null" {
        ui::warn(&format!("A custom Private DNS is already set ({spec}) - left as is"));
        return;
    }
    if !resolves(tw.adb) {
        ui::warn("The phone has no internet right now - skipped. Connect to Wi-Fi and run again.");
        return;
    }
    if !tw.set("global", "private_dns_specifier", dns_host) || !tw.set("global", "private_dns_mode", "hostname") {
        ui::warn("The phone did not allow changing DNS");
        restore_setting(tw.adb, "global", "private_dns_mode", &mode);
        return;
    }
    for _ in 0..5 {
        sleep(Duration::from_secs(3));
        if resolves(tw.adb) {
            ui::ok(&format!("On: {dns_host} (ads and trackers are blocked for the whole phone)"));
            return;
        }
    }
    restore_setting(tw.adb, "global", "private_dns_mode", &mode);
    restore_setting(tw.adb, "global", "private_dns_specifier", &spec);
    ui::warn(&format!("DNS server {dns_host} is unreachable from this network - rolled back, the internet works"));
}

fn apply_speed_settings(tw: &mut Tweaks, info: &DeviceInfo) {
    let anim = ["window_animation_scale", "transition_animation_scale", "animator_duration_scale"]
        .iter()
        .filter(|k| tw.set("global", k, config::ANIMATION_SCALE))
        .count();
    if anim == 3 {
        ui::ok("Animations 2x faster - the interface feels snappier");
    }
    if tw.set("global", "wifi_scan_always_enabled", "0") && tw.set("global", "ble_scan_always_enabled", "0") {
        ui::ok("Constant background Wi-Fi/Bluetooth scanning off - saves battery (location still works)");
    }
    if tw.set("global", "mobile_data_always_on", "0") {
        ui::ok("Mobile data is not kept on while on Wi-Fi - saves battery");
    }
    if matches!(info.brand, crate::brands::Brand::Xiaomi)
        && tw.set("global", "com.miui.home.setting_recommend_app", "0")
    {
        ui::ok("Xiaomi home screen app recommendations off");
    }
}

fn apply_text_size(tw: &mut Tweaks, plan: &Plan) {
    if plan.font > 0.0 && tw.set("system", "font_scale", &format!("{}", plan.font)) {
        ui::ok(&format!("Text size x{}", plan.font));
    }
    if plan.display > 0.0 && tw.set_display_scale(plan.display) {
        ui::ok(&format!("Display size x{} - text, buttons and icons are bigger", plan.display));
    }
}

fn media_packages(paths: &Paths) -> Vec<String> {
    lists::read_list(&paths.list_file("background")).into_iter().filter(|s| s.media).map(|s| s.pattern).collect()
}

fn apply_background(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan, paths: &Paths) {
    let limited: Vec<String> = plan.background.iter().filter(|p| tw.set_background_restricted(p)).cloned().collect();
    if limited.is_empty() {
        return;
    }
    ui::ok(&format!("Background activity limited: {}", limited.join(", ")));
    if !matches!(info.brand, crate::brands::Brand::Xiaomi) {
        return;
    }
    let media = media_packages(paths);
    let (mut ok, mut fail) = (Vec::new(), Vec::new());
    ui::dim("Xiaomi: setting the limit in Xiaomi's own settings (do not touch the phone for ~1 minute)...");
    for p in limited.iter().filter(|p| !media.contains(p)) {
        if screen::set_xiaomi_background_limit(tw.adb, p) {
            ok.push(p.clone());
        } else {
            fail.push(p.clone());
        }
    }
    let _ = tw.adb.sh("input keyevent KEYCODE_HOME");
    if !ok.is_empty() {
        ui::ok(&format!("Xiaomi: \"close after 10 minutes in the background\" - {} (survives reboots)", ok.join(", ")));
    }
    if !fail.is_empty() {
        ui::warn(&format!("Xiaomi resets the background limit on reboot. Set it by hand for {}:", fail.join(", ")));
        ui::dim("App info -> Battery -> \"Close apps after 10 minutes of background activity\".");
    }
}

fn apply_mute(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan) {
    let muted: Vec<String> = plan.mute.iter().filter(|p| tw.set_notifications_blocked(p, info.sdk)).cloned().collect();
    if !muted.is_empty() {
        ui::ok(&format!("Notifications off (and the apps cannot ask again): {}", muted.join(", ")));
    }
}

pub fn apply_tweaks(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan, paths: &Paths) {
    ui::step("Smoothness and battery settings");
    apply_speed_settings(tw, info);
    apply_text_size(tw, plan);
    apply_background(tw, info, plan, paths);
    apply_mute(tw, info, plan);
    if info.has_accounts && screen::set_ad_personalization_off(tw.adb) {
        ui::ok("Google ad personalization off (ads no longer follow your interests)");
    }
}

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
                if m == "disabled" {
                    ui::ok(&format!("{name} (disabled)"));
                } else {
                    ui::ok(name);
                }
            }
            None => ui::warn(&format!("{name}: {}", r.trim())),
        }
    }
    count
}

const COMPILE_SCRIPT: &str = include_str!("../tools/compile-apps.sh");
const COMPILE_REMOTE: &str = "/data/local/tmp/clearphone-compile.sh";
/// "Free everything" for `pm trim-caches`: 1 TiB.
const TRIM_ALL_BYTES: &str = "1099511627776";
/// Compiling on a nearly empty battery risks the phone dying mid-run.
const COMPILE_MIN_BATTERY: i64 = 20;
/// Android 7+ has the speed compiler this relies on.
const COMPILE_MIN_SDK: i32 = 24;

/// One line of compile-apps.sh output, "12/47 OK com.pkg" or "12/47 -- com.pkg".
pub struct CompileStep {
    pub index: usize,
    pub total: usize,
    pub ok: bool,
    pub package: String,
}

pub fn parse_compile_line(line: &str) -> Option<CompileStep> {
    let mut it = line.split_whitespace();
    let (index, total) = it.next()?.split_once('/')?;
    let ok = match it.next()? {
        "OK" => true,
        "--" => false,
        _ => return None,
    };
    Some(CompileStep { index: index.parse().ok()?, total: total.parse().ok()?, ok, package: it.next()?.to_string() })
}

/// Compile only the apps the person opens (plus System UI and the home screen) - compiling every system
/// component would take hours on budget chips. Returns how many were compiled.
fn full_compile(adb: &Adb) -> usize {
    let tmp = std::env::temp_dir().join("clearphone-compile.sh");
    if std::fs::write(&tmp, COMPILE_SCRIPT.replace("\r\n", "\n")).is_err() {
        return 0;
    }
    adb.push(&tmp, COMPILE_REMOTE);
    let mut compiled = 0;
    adb.shell_stream(&format!("sh {COMPILE_REMOTE}"), |line| {
        if let Some(s) = parse_compile_line(line) {
            compiled += usize::from(s.ok);
            let mark = if s.ok { "" } else { " (skipped)" };
            ui::dim(&format!(
                "{} {}/{} {}{mark}",
                ui::bar(s.index as f64 / s.total.max(1) as f64),
                s.index,
                s.total,
                s.package
            ));
        }
    });
    let _ = adb.sh(&format!("rm -f {COMPILE_REMOTE}"));
    let _ = std::fs::remove_file(&tmp);
    compiled
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

fn enough_space_for_full_compile(adb: &Adb) -> bool {
    diagnostics::storage(adb).is_some_and(|s| {
        s.free_mb >= config::COMPILE_MIN_FREE_MB
            && s.free_mb as f64 >= s.total_mb as f64 * config::COMPILE_MIN_FREE_FRACTION
    })
}

/// Ahead-of-time compile apps so they start and run faster.
pub fn optimize_apps(adb: &Adb, info: &DeviceInfo, diag: &Diag) {
    ui::step("App optimization (compiling)");
    if info.sdk < COMPILE_MIN_SDK {
        ui::warn("Needs Android 7 or newer - skipped");
    } else if diag.battery_level.is_some_and(|l| l < COMPILE_MIN_BATTERY) && !diag.charging {
        ui::warn("Battery below 20% - app optimization skipped (charge the phone and run again)");
    } else if enough_space_for_full_compile(adb) {
        ui::say("Fully compiling every app with an icon - they will start and run faster.");
        ui::dim("Up to an hour on budget phones. Keep the phone connected...");
        ui::ok(&format!("Fully compiled apps: {}", full_compile(adb)));
    } else {
        ui::warn("Not enough free storage for full compilation - running the standard optimization");
        ui::say("Optimizing (compiling) apps - this may take 5-20 minutes...");
        let _ = adb.sh("cmd package bg-dexopt-job");
        ui::ok("Apps optimized");
    }
}

pub fn stop_background(adb: &Adb) {
    let _ = adb.sh("am kill-all");
    ui::ok("Background processes stopped");
}
