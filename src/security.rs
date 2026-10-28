//! Signs of malware and spyware that are visible without root. Reports only; changes nothing.
//!
//! Most preinstalled malware uses random, ever-changing package names, so the heuristics below matter
//! more than any fixed list. `lists/malware.txt` only covers stable, documented families.

use crate::adb::Adb;
use crate::apps;
use crate::config::APK_INSTALLERS;
use crate::device::{is_oem, DeviceInfo, Packages, Phone};
use crate::lists;
use crate::paths::Paths;
use crate::perms::{self, PowerApps};
use crate::root::{self, Root};
use crate::ui;
use std::collections::HashSet;
use std::path::Path;

#[derive(Default)]
pub struct Findings {
    /// Serious: likely malware or a compromised system.
    pub issues: Vec<String>,
    /// Worth a look, usually fine.
    pub notes: Vec<String>,
}

/// Run every check, print the result and save it to the report. Returns the number of serious issues.
pub fn run(phone: &Phone, pkgs: &Packages, paths: &Paths, report_dir: &Path) -> usize {
    ui::step("Security check (signs of viruses and spyware)");
    let f = audit(phone, pkgs, paths);
    show(&f);
    save(&f, report_dir);
    f.issues.len()
}

pub fn audit(phone: &Phone, pkgs: &Packages, paths: &Paths) -> Findings {
    let adb = &phone.adb;
    let mut f = Findings::default();
    f.issues.extend(known_malware(pkgs, paths));
    f.issues.extend(system_integrity(adb, &phone.info));
    f.issues.extend(app_origins(adb, pkgs, &apps::listed_packages(paths, &phone.info, pkgs)));
    f.issues.extend(
        parse_admins(&adb.sh("dumpsys device_policy"))
            .into_iter()
            .filter(|p| pkgs.is_third_party(p))
            .map(|p| format!("{p} - device administrator (malware uses this to block removal).")),
    );
    permission_findings(&perms::power_apps(adb, pkgs), &mut f);
    network_findings(adb, &mut f);
    if let Some(root) = &phone.root {
        root_findings(root, adb, &mut f);
    }
    f
}

fn known_malware(pkgs: &Packages, paths: &Paths) -> Vec<String> {
    lists::read_patterns(&paths.list_file("malware"))
        .iter()
        .flat_map(|pat| pkgs.matching(pat))
        .filter(|name| pkgs.get(name).map(|p| p.enabled).unwrap_or(false))
        .map(|name| format!("{name} - KNOWN preinstalled malware/riskware. Optimize removes it, or remove it by hand."))
        .collect()
}

fn system_integrity(adb: &Adb, info: &DeviceInfo) -> Vec<String> {
    let mut v = Vec::new();
    if !info.boot_state.is_empty() && info.boot_state != "green" {
        v.push(format!(
            "The firmware is modified or the bootloader is unlocked (verified boot: {}). Dangerous if you did not do it.",
            info.boot_state
        ));
    }
    if !adb.sh("getenforce").contains("Enforcing") {
        v.push("SELinux protection is off - a sign of a compromised system.".into());
    }
    if adb.get_setting("global", "package_verifier_enable") == "0" {
        v.push("Play Protect is off: Play Store -> profile icon -> Play Protect -> turn it on.".into());
    }
    v
}

/// Packages that have a home-screen icon, from `cmd package query-activities --brief` output.
pub fn parse_launchable(dump: &str) -> HashSet<String> {
    dump.lines().filter_map(|l| l.trim().split_once('/').map(|(p, _)| p.trim().to_string())).collect()
}

fn app_origins(adb: &Adb, pkgs: &Packages, listed: &HashSet<String>) -> Vec<String> {
    let launchable =
        parse_launchable(&adb.sh(
            "cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER",
        ));
    origin_findings(pkgs, &launchable, listed)
}

/// Where third-party apps came from. Preinstalls named in the lists (`listed`) are skipped: makers ship them
/// without an installer, and the removal plan already covers them.
pub fn origin_findings(pkgs: &Packages, launchable: &HashSet<String>, listed: &HashSet<String>) -> Vec<String> {
    let mut v = Vec::new();
    // An APK install is suspicious even under an OEM-looking name; the other two signals are normal for OEM apps.
    for p in pkgs.values().filter(|p| !p.system && p.enabled && !listed.contains(&p.name)) {
        let oem = is_oem(&p.name);
        if APK_INSTALLERS.contains(&p.installer.as_str()) {
            v.push(format!(
                "{} - installed from an APK file, not a store. If you did not install it, remove it.",
                p.name
            ));
        } else if p.installer.is_empty() && !oem {
            v.push(format!("{} - unknown source.", p.name));
        }
        if !launchable.contains(&p.name) && !oem {
            v.push(format!("{} - app without an icon (hidden). Find out what it is.", p.name));
        }
    }
    v
}

/// Device-admin packages from `dumpsys device_policy`: the "Enabled Device Admins" block ends where the
/// indentation returns to the header's level (the next block, e.g. "Device Owner:", starts there).
pub fn parse_admins(dump: &str) -> Vec<String> {
    let mut admins = Vec::new();
    let mut header_indent: Option<usize> = None;
    for line in dump.lines().filter(|l| !l.trim().is_empty()) {
        let indent = line.len() - line.trim_start().len();
        if line.contains("Enabled Device Admins") {
            header_indent = Some(indent);
        } else if header_indent.is_some_and(|h| indent <= h) {
            header_indent = None;
        } else if header_indent.is_some() {
            if let Some((pkg, _)) = line.trim().strip_suffix(':').and_then(|c| c.split_once('/')) {
                admins.push(pkg.to_string());
            }
        }
    }
    admins
}

fn permission_findings(apps: &PowerApps, f: &mut Findings) {
    for p in &apps.accessibility {
        f.issues.push(format!(
            "{p} - uses Accessibility: sees the screen and can tap for you (banking trojans do this). Turn it off if you did not enable it."
        ));
    }
    for p in &apps.installers {
        f.issues.push(format!("{p} - may install other apps. Turn it off in the app settings if not needed."));
    }
    for p in &apps.notification_listeners {
        f.notes
            .push(format!("{p} reads all notifications, including SMS codes (normal for watches and fitness bands)"));
    }
    for p in &apps.overlay {
        f.notes.push(format!("{p} may draw over other apps"));
    }
}

fn network_findings(adb: &Adb, f: &mut Findings) {
    let proxy = adb.get_setting("global", "http_proxy");
    if !proxy.is_empty() && proxy != "null" && proxy != ":0" {
        f.issues.push(format!("All traffic goes through proxy {proxy} - dangerous if you did not set it up."));
    }
    let vpn = adb.get_setting("secure", "always_on_vpn_app");
    if !vpn.is_empty() && vpn != "null" {
        f.notes.push(format!("always-on VPN: {vpn} - make sure you know it"));
    }
}

/// Only for phones the owner rooted: "is the phone deliberately throttled?"
fn root_findings(root: &Root, adb: &Adb, f: &mut Findings) {
    f.notes.push("this phone is rooted (Magisk or similar). Fine if you did it; a risk if you did not".into());
    f.issues.extend(root::cpu_caps(root, adb));
}

pub fn show(f: &Findings) {
    if f.issues.is_empty() {
        ui::ok("No signs of viruses or spyware");
        ui::dim("firmware not modified; no APKs from outside a store, hidden apps, foreign device admins,");
        ui::dim("Accessibility abuse, install permissions or traffic interception (proxy)");
    }
    for i in &f.issues {
        ui::warn(i);
    }
    if !f.notes.is_empty() {
        ui::dim("Worth a look (usually fine):");
        for n in &f.notes {
            ui::dim(&format!("- {n}"));
        }
    }
    ui::dim("You can also run: Play Store -> profile icon -> Play Protect -> Scan.");
}

fn save(f: &Findings, dir: &Path) {
    let mut body: Vec<String> = f.issues.clone();
    body.extend(f.notes.iter().map(|n| format!("note: {n}")));
    let _ = std::fs::write(dir.join("security.txt"), body.join("\n"));
}
