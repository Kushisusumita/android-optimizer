//! Settings that "optimizer" apps and online tips leave behind and that make a phone lag, restart apps
//! or drain the battery. Only values someone changed away from Android's default count; an unset value
//! ("null") is the default and is left alone. Keys are from AOSP `Settings.java`.

use crate::phone::adb::Adb;

pub struct Leftover {
    pub ns: &'static str,
    pub key: &'static str,
    /// The value that undoes the damage.
    pub healthy: &'static str,
    /// What the bad value did, for the person.
    pub what: &'static str,
    is_bad: fn(&str) -> bool,
}

const KNOWN: &[Leftover] = &[
    Leftover {
        ns: "global",
        key: "always_finish_activities",
        healthy: "0",
        what: "\"Don't keep activities\" was on - every app restarted when you came back to it",
        is_bad: |v| v == "1",
    },
    Leftover {
        ns: "global",
        key: "stay_on_while_plugged_in",
        healthy: "0",
        what: "the screen never turned off while charging",
        is_bad: |v| v.parse::<i32>().is_ok_and(|n| n != 0),
    },
    Leftover {
        ns: "global",
        key: "adaptive_battery_management_enabled",
        healthy: "1",
        what: "Adaptive Battery was off - apps used in the background were not limited",
        is_bad: |v| v == "0",
    },
    Leftover {
        ns: "global",
        key: "app_standby_enabled",
        healthy: "1",
        what: "app standby was off - apps you do not use kept running",
        is_bad: |v| v == "0",
    },
    Leftover {
        ns: "global",
        key: "app_auto_restriction_enabled",
        healthy: "1",
        what: "automatic limits for battery-draining apps were off",
        is_bad: |v| v == "0",
    },
    Leftover {
        ns: "global",
        key: "cached_apps_freezer",
        healthy: "device_default",
        what: "the freezer for apps in the background was turned off",
        is_bad: |v| v == "disabled",
    },
];

/// The leftovers present, given a way to read a setting (`read(ns, key)`).
pub fn found(read: impl Fn(&str, &str) -> String) -> Vec<&'static Leftover> {
    KNOWN.iter().filter(|l| (l.is_bad)(read(l.ns, l.key).trim())).collect()
}

pub fn on_phone(adb: &Adb) -> Vec<&'static Leftover> {
    found(|ns, key| adb.get_setting(ns, key))
}
