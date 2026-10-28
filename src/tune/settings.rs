//! Reversible `settings put` / `wm density` changes: each original value is recorded once for Restore.

use super::{Change, Plan};
use crate::config;
use crate::host::report::Report;
use crate::phone::adb::Adb;
use crate::phone::info::parse_wm_size;
use crate::ui;
use crate::util::num_after;
use std::collections::HashSet;

/// Used when `wm size` cannot be read: a typical phone's short side.
const DEFAULT_SMALLEST_SIDE_PX: i64 = 1080;

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

/// Proof that this phone accepts settings changes. Only `check` can make one, so nothing can try to change
/// settings on a phone that locks them.
pub struct Access(());

impl Access {
    pub fn check(adb: &Adb) -> Option<Access> {
        test_writable(adb).then_some(Access(()))
    }
}

/// Reversible changes to one phone during one run.
pub struct Tweaks<'a> {
    pub(super) adb: &'a Adb,
    pub(super) report: &'a Report,
    saved: HashSet<String>,
    /// The phone refused at least one change (its settings are locked).
    pub secure_denied: bool,
}

impl<'a> Tweaks<'a> {
    pub fn new(adb: &'a Adb, report: &'a Report, _access: &Access) -> Self {
        Tweaks { adb, report, saved: HashSet::new(), secure_denied: false }
    }

    fn save_original(&mut self, ns: &str, key: &str, old: &str) {
        if self.saved.insert(format!("{ns}/{key}")) {
            self.report.append("tweaks.tsv", &format!("{ns}\t{key}\t{old}"));
        }
    }

    /// Run a changing command; false (and the phone marked as locked) if it was refused.
    pub(super) fn change(&mut self, cmd: &str) -> bool {
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
}

/// Can we change settings at all? (Xiaomi blocks this without "USB debugging (Security settings)".)
fn test_writable(adb: &Adb) -> bool {
    if refused(&adb.sh("settings put global clearphone_probe 1")) {
        return false;
    }
    let _ = adb.sh("settings delete global clearphone_probe");
    true
}

/// Put back one recorded original value.
pub fn restore_setting(adb: &Adb, ns: &str, key: &str, value: &str) {
    let unset = value.is_empty() || value == "null" || value == "reset";
    let cmd = match (ns, unset) {
        ("wm", true) => format!("wm {key} reset"),
        ("wm", false) => format!("wm {key} {value}"),
        (_, true) => format!("settings delete {ns} {key}"),
        (_, false) => format!("settings put {ns} {key} {value}"),
    };
    let _ = adb.sh(&cmd);
}

const ANIMATION_KEYS: [&str; 3] = ["window_animation_scale", "transition_animation_scale", "animator_duration_scale"];

pub(super) fn apply_speed_settings(tw: &mut Tweaks, plan: &Plan) {
    if plan.wants(Change::Animations)
        && ANIMATION_KEYS.iter().filter(|k| tw.set("global", k, config::ANIMATION_SCALE)).count()
            == ANIMATION_KEYS.len()
    {
        ui::ok("Animations 2x faster - the interface feels snappier");
    }
    if plan.wants(Change::Scanning)
        && tw.set("global", "wifi_scan_always_enabled", "0")
        && tw.set("global", "ble_scan_always_enabled", "0")
    {
        ui::ok("Constant background Wi-Fi/Bluetooth scanning off - saves battery (location still works)");
    }
    if plan.wants(Change::MobileData) && tw.set("global", "mobile_data_always_on", "0") {
        ui::ok("Mobile data is not kept on while on Wi-Fi - saves battery");
    }
    if plan.wants(Change::XiaomiRecommendations) && tw.set("global", "com.miui.home.setting_recommend_app", "0") {
        ui::ok("Xiaomi home screen app recommendations off");
    }
}

pub(super) fn apply_text_size(tw: &mut Tweaks, plan: &Plan) {
    if plan.font > 0.0 && tw.set("system", "font_scale", &format!("{}", plan.font)) {
        ui::ok(&format!("Text size x{}", plan.font));
    }
    if plan.display > 0.0 && tw.set_display_scale(plan.display) {
        ui::ok(&format!("Display size x{} - text, buttons and icons are bigger", plan.display));
    }
}
