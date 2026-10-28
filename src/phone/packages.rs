//! The installed package set from `pm list packages`, parsed once into `Packages`.

use super::adb::Adb;
use std::collections::{BTreeMap, HashSet};

const LAUNCHER_QUERY: &str =
    "cmd package query-activities --brief -a android.intent.action.MAIN -c android.intent.category.LAUNCHER";

/// Packages with a home-screen icon, from `cmd package query-activities --brief` output.
pub fn parse_launchable(dump: &str) -> HashSet<String> {
    dump.lines().filter_map(|l| l.trim().split_once('/').map(|(p, _)| p.trim().to_string())).collect()
}

/// The apps a person can open from the home screen.
pub fn launchable(adb: &Adb) -> HashSet<String> {
    parse_launchable(&adb.sh(LAUNCHER_QUERY))
}

/// Package-name prefixes that belong to Android itself, Google, or a device maker (installed without a
/// store - that is normal, not a malware signal).
const OEM_COM: &[&str] = &[
    "android",
    "google",
    "miui",
    "xiaomi",
    "mi",
    "samsung",
    "sec",
    "huawei",
    "hihonor",
    "oppo",
    "coloros",
    "heytap",
    "oplus",
    "nearme",
    "vivo",
    "bbk",
    "iqoo",
    "transsion",
    "motorola",
    "mediatek",
    "qualcomm",
    "qti",
    "lbe",
    "duokan",
];

pub fn is_oem(name: &str) -> bool {
    const OTHER: &[&str] = &["android.", "vendor.", "cn.wps.xiaomi.", "org.codeaurora."];
    OTHER.iter().any(|p| name.starts_with(p)) || OEM_COM.iter().any(|c| name.starts_with(&format!("com.{c}.")))
}

/// One installed package.
pub struct Pkg {
    pub name: String,
    pub installer: String,
    pub system: bool,
    pub enabled: bool,
}

/// The installed package set, keyed by package name.
pub struct Packages {
    map: BTreeMap<String, Pkg>,
}

fn listed_names(output: &str) -> impl Iterator<Item = &str> {
    output.lines().filter_map(|l| l.trim().strip_prefix("package:")).map(|r| r.split_whitespace().next().unwrap_or(""))
}

impl Packages {
    pub fn load(adb: &Adb) -> Packages {
        Packages::parse(&adb.sh("pm list packages -i"), &adb.sh("pm list packages -s"), &adb.sh("pm list packages -d"))
    }

    /// Build the set from `pm list packages -i`, `-s` (system) and `-d` (disabled) output.
    pub fn parse(with_installers: &str, system: &str, disabled: &str) -> Packages {
        let mut map = BTreeMap::new();
        for line in with_installers.lines() {
            let Some(rest) = line.trim().strip_prefix("package:") else { continue };
            let mut parts = rest.split_whitespace();
            let Some(name) = parts.next() else { continue };
            let installer =
                parts.find_map(|t| t.strip_prefix("installer=")).filter(|i| *i != "null").unwrap_or("").to_string();
            map.insert(name.to_string(), Pkg { name: name.to_string(), installer, system: false, enabled: true });
        }
        for name in listed_names(system) {
            if let Some(p) = map.get_mut(name) {
                p.system = true;
            }
        }
        for name in listed_names(disabled) {
            if let Some(p) = map.get_mut(name) {
                p.enabled = false;
            }
        }
        Packages { map }
    }

    pub fn get(&self, name: &str) -> Option<&Pkg> {
        self.map.get(name)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }

    /// A real third-party app: installed, not a system app, and not an OEM/Google package.
    pub fn is_third_party(&self, name: &str) -> bool {
        self.get(name).map(|p| !p.system && !is_oem(name)).unwrap_or(false)
    }

    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.map.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &Pkg> {
        self.map.values()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn user_app_count(&self) -> usize {
        self.map.values().filter(|p| !p.system).count()
    }

    /// Installed names matching a list pattern (exact name or a glob with one `*`).
    pub fn matching(&self, pattern: &str) -> Vec<String> {
        if !pattern.contains('*') {
            return if self.contains(pattern) { vec![pattern.to_string()] } else { Vec::new() };
        }
        self.map.keys().filter(|n| name_matches(pattern, n)).cloned().collect()
    }
}

/// A package name against a list pattern: an exact name, or a glob with one `*`.
pub fn name_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        Some((pre, post)) => name.len() >= pre.len() + post.len() && name.starts_with(pre) && name.ends_with(post),
        None => pattern == name,
    }
}
