//! Settings adb cannot reach, changed by tapping through the phone's own screens as a person would:
//! Xiaomi's per-app battery mode and Google's ad personalization switch.

use crate::phone::adb::Adb;
use crate::phone::screen::{dump_nodes, scroll, tap_node, UiNode};
use std::thread::sleep;
use std::time::Duration;

/// Xiaomi (PowerKeeper) resets the standard Android background limit on every boot and hides its own
/// setting from adb, so it is set through App info -> Battery, as a person would. The "Close apps after
/// 10 minutes" mode survives reboots. "Soft restriction" is never used - despite its wording it stops
/// ALL background work (music stops).
pub fn set_xiaomi_background_limit(adb: &Adb, pkg: &str) -> bool {
    let battery_labels = ["Battery", "Battery saver", "Battery usage"];
    let mode_labels = ["Close apps after 10 minutes"];
    let no_limit_labels = ["No restrictions"];

    let _ =
        adb.sh(&format!("am start -f 0x10008000 -a android.settings.APPLICATION_DETAILS_SETTINGS -d package:{pkg}"));
    sleep(Duration::from_secs(3));
    let bat = dump_nodes(adb)
        .into_iter()
        .find(|n| n.resource_id == "android:id/title" && battery_labels.contains(&n.text.as_str()));
    match bat {
        Some(b) if tap_node(adb, &b) => {}
        _ => return false,
    }
    sleep(Duration::from_millis(2500));
    for _ in 0..2 {
        let titles: Vec<UiNode> = dump_nodes(adb).into_iter().filter(|n| n.resource_id == "android:id/title").collect();
        let mode = titles.iter().position(|n| mode_labels.iter().any(|m| n.text.starts_with(m))).or_else(|| {
            // Unknown language: the option two rows after "No restrictions".
            titles.iter().position(|n| no_limit_labels.contains(&n.text.as_str())).and_then(|i| {
                if i + 2 < titles.len() {
                    Some(i + 2)
                } else {
                    None
                }
            })
        });
        if let Some(idx) = mode {
            let node = &titles[idx];
            if node.checked {
                return true;
            }
            tap_node(adb, node);
            sleep(Duration::from_secs(2));
            let nodes = dump_nodes(adb);
            if let Some(cancel) = nodes.iter().find(|n| n.resource_id == "android:id/button2") {
                // Unexpected confirmation - do not guess, cancel and report failure.
                tap_node(adb, cancel);
                return false;
            }
            return nodes.iter().any(|n| {
                n.resource_id == "android:id/title" && n.checked && mode_labels.iter().any(|m| n.text.starts_with(m))
            });
        }
        scroll(adb);
    }
    false
}

/// Google "Opt out of Ads Personalization" on Play services screens that still have a switch.
/// Newer screens only offer deleting the advertising ID; that is left to the person.
pub fn set_ad_personalization_off(adb: &Adb) -> bool {
    let labels = ["Opt out of Ads Personalization", "Opt out of ads personalization"];
    let _ = adb.sh("am start -f 0x10008000 -a com.google.android.gms.settings.ADS_PRIVACY");
    sleep(Duration::from_secs(3));
    let nodes = dump_nodes(adb);
    let has_switch_on = nodes.iter().any(|n| n.class.contains("Switch") && n.checked);
    let row = nodes.iter().find(|n| labels.contains(&n.text.as_str()));
    let mut result = false;
    if nodes.iter().any(|n| n.class.contains("Switch")) {
        if has_switch_on {
            result = true;
        } else if let Some(row) = row {
            tap_node(adb, row);
            sleep(Duration::from_secs(2));
            if let Some(ok) = dump_nodes(adb).iter().find(|n| n.resource_id == "android:id/button1") {
                tap_node(adb, ok);
                sleep(Duration::from_secs(2));
            }
            result = dump_nodes(adb).iter().any(|n| n.class.contains("Switch") && n.checked);
        }
    }
    let _ = adb.sh("input keyevent KEYCODE_HOME");
    result
}
