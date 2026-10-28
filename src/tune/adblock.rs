//! System-wide ad and tracker blocking through Private DNS (strict mode). Verified after switching on,
//! and rolled back if the server is unreachable, so the internet never breaks.

use super::settings::{restore_setting, Tweaks};
use crate::phone::adb::Adb;
use crate::phone::info::DeviceInfo;
use crate::ui;
use std::thread::sleep;
use std::time::Duration;

/// Private DNS needs Android 9.
const PRIVATE_DNS_SDK: i32 = 28;
const PROBE_HOSTS: [&str; 2] = ["google.com", "ya.ru"];
const VERIFY_TRIES: usize = 5;
const VERIFY_PAUSE: Duration = Duration::from_secs(3);

fn resolves(adb: &Adb) -> bool {
    PROBE_HOSTS.iter().any(|h| adb.sh(&format!("ping -c 1 -W 2 {h}")).contains("PING "))
}

pub fn enable(tw: &mut Tweaks, info: &DeviceInfo, dns_host: &str) {
    ui::step("Ad and tracker blocking in all apps and browsers (Private DNS)");
    if info.sdk < PRIVATE_DNS_SDK {
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
    if (0..VERIFY_TRIES).any(|_| {
        sleep(VERIFY_PAUSE);
        resolves(tw.adb)
    }) {
        ui::ok(&format!("On: {dns_host} (ads and trackers are blocked for the whole phone)"));
        return;
    }
    restore_setting(tw.adb, "global", "private_dns_mode", &mode);
    restore_setting(tw.adb, "global", "private_dns_specifier", &spec);
    ui::warn(&format!("DNS server {dns_host} is unreachable from this network - rolled back, the internet works"));
}
