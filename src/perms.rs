//! Which third-party apps hold the powerful permissions that adware and spyware abuse.
//! Shared by the security check and the pop-up ad finder.

use crate::adb::Adb;
use crate::device::Packages;

pub struct PowerApps {
    /// May draw windows over every other app - the usual source of pop-up ads.
    pub overlay: Vec<String>,
    /// Accessibility services: see the screen and tap for you (banking trojans do this).
    pub accessibility: Vec<String>,
    /// Read every notification, including SMS codes.
    pub notification_listeners: Vec<String>,
    /// May install other apps.
    pub installers: Vec<String>,
}

/// Package names from a component list setting such as "com.a/.Svc:com.b/com.b.Other".
pub fn component_packages(setting: &str) -> Vec<String> {
    setting
        .split(':')
        .filter_map(|c| c.split_once('/').map(|(p, _)| p.trim().to_string()))
        .filter(|p| !p.is_empty())
        .collect()
}

/// Package names from `cmd appops query-op <op> allow` output (one per line).
pub fn appops_packages(output: &str) -> Vec<String> {
    output.lines().map(str::trim).filter(|l| !l.is_empty() && !l.contains(' ')).map(String::from).collect()
}

fn third_party_only(names: Vec<String>, pkgs: &Packages) -> Vec<String> {
    let mut v: Vec<String> = names.into_iter().filter(|p| pkgs.is_third_party(p)).collect();
    v.sort();
    v.dedup();
    v
}

pub fn power_apps(adb: &Adb, pkgs: &Packages) -> PowerApps {
    let appops = |op: &str| appops_packages(&adb.sh(&format!("cmd appops query-op {op} allow")));
    let components = |key: &str| component_packages(&adb.get_setting("secure", key));
    PowerApps {
        overlay: third_party_only(appops("SYSTEM_ALERT_WINDOW"), pkgs),
        accessibility: third_party_only(components("enabled_accessibility_services"), pkgs),
        notification_listeners: third_party_only(components("enabled_notification_listeners"), pkgs),
        installers: third_party_only(appops("REQUEST_INSTALL_PACKAGES"), pkgs),
    }
}
