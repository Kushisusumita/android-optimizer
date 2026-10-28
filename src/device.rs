//! The connected phone: its properties (getprop), the installed package set (pm list) and whether the
//! owner rooted it. Raw adb output is parsed here, once; the rest of the code trusts these types.

use crate::adb::Adb;
use crate::brands::{Brand, Props};
use crate::lists;
use crate::root::Root;
use std::collections::BTreeMap;

/// Everything the actions need about the phone in one value.
pub struct Phone {
    pub adb: Adb,
    pub info: DeviceInfo,
    pub root: Option<Root>,
}

pub struct DeviceInfo {
    pub device_id: String,
    pub brand: Brand,
    pub brand_name: String,
    pub model: String,
    pub name: String,
    pub rom: String,
    /// System-on-chip, e.g. "Mediatek MT6769Z" (empty when the firmware does not say).
    pub soc: String,
    pub android: String,
    pub sdk: i32,
    pub patch: String,
    pub fingerprint: String,
    pub boot_state: String,
    pub has_accounts: bool,
}

/// Parse `getprop` output: lines like `[ro.product.model]: [SM-A155F]`.
pub fn parse_getprop(text: &str) -> Props {
    let mut props = Props::new();
    for line in text.lines() {
        if let Some((k, v)) = line.trim().strip_prefix('[').and_then(|rest| rest.split_once("]: [")) {
            props.insert(k.to_string(), v.strip_suffix(']').unwrap_or(v).to_string());
        }
    }
    props
}

/// `wm size` -> (width, height) of the physical screen in pixels.
pub fn parse_wm_size(text: &str) -> Option<(i64, i64)> {
    let (w, h) = text.split("Physical size:").nth(1)?.trim().split_once('x')?;
    Some((w.trim().parse().ok()?, h.split_whitespace().next()?.parse().ok()?))
}

/// The number after "Accounts:" in `dumpsys account`.
pub fn accounts_count(dump: &str) -> usize {
    dump.lines().find_map(|l| l.trim().strip_prefix("Accounts:")).and_then(|n| n.trim().parse().ok()).unwrap_or(0)
}

fn first(props: &Props, keys: &[&str]) -> String {
    keys.iter().filter_map(|k| props.get(*k)).find(|v| !v.is_empty()).cloned().unwrap_or_default()
}

const MARKET_NAME_KEYS: &[&str] = &[
    "ro.product.marketname",
    "ro.product.vendor.marketname",
    "ro.product.odm.marketname",
    "ro.config.marketing_name",
    "ro.oppo.market.name",
    "ro.vivo.market.name",
];

impl DeviceInfo {
    pub fn load(adb: &Adb) -> DeviceInfo {
        let props = parse_getprop(&adb.sh("getprop"));
        let has_accounts = accounts_count(&adb.sh("dumpsys account")) > 0;
        DeviceInfo::from_props(&props, adb.serial().unwrap_or(""), has_accounts)
    }

    pub fn from_props(props: &Props, serial: &str, has_accounts: bool) -> DeviceInfo {
        let brand_name = first(props, &["ro.product.brand", "ro.product.vendor.brand"]);
        let manufacturer = first(props, &["ro.product.manufacturer", "ro.product.vendor.manufacturer"]);
        let model = first(props, &["ro.product.model", "ro.product.vendor.model"]);
        let mut name = first(props, MARKET_NAME_KEYS);
        if name.is_empty() {
            name = format!("{brand_name} {model}").trim().to_string();
        }
        let mut device_id = first(props, &["ro.serialno", "ro.boot.serialno"]);
        if device_id.is_empty() {
            device_id = serial.to_string();
        }
        DeviceInfo {
            device_id,
            brand: Brand::detect(&brand_name, &manufacturer, props),
            brand_name,
            model,
            name,
            rom: detect_rom(props),
            soc: format!(
                "{} {}",
                first(props, &["ro.soc.manufacturer"]),
                first(props, &["ro.soc.model", "ro.hardware.chipname", "ro.board.platform", "ro.hardware"])
            )
            .trim()
            .to_string(),
            android: first(props, &["ro.build.version.release"]),
            sdk: first(props, &["ro.build.version.sdk"]).parse().unwrap_or(0),
            patch: first(props, &["ro.build.version.security_patch"]),
            fingerprint: first(props, &["ro.build.fingerprint"]),
            boot_state: first(props, &["ro.boot.verifiedbootstate"]),
            has_accounts,
        }
    }

    /// List file stems to load for debloat: common + google + this brand (malware is loaded separately).
    pub fn list_stems(&self) -> Vec<&'static str> {
        let mut v = vec!["common", "google"];
        v.extend(self.brand.list_stem());
        v
    }
}

fn detect_rom(p: &Props) -> String {
    let get = |k: &str| p.get(k).cloned().unwrap_or_default();
    if let Some(v) = p.get("ro.mi.os.version.incremental") {
        format!("HyperOS {v}")
    } else if let Some(v) = p.get("ro.miui.ui.version.name") {
        format!("MIUI {v} ({})", get("ro.build.version.incremental"))
    } else if let Some(v) = p.get("ro.build.version.oneui").and_then(|s| s.parse::<i64>().ok()) {
        format!("One UI {}.{}", v / 10000, (v % 10000) / 100)
    } else if let Some(v) = p.get("ro.build.version.oplusrom").or_else(|| p.get("ro.build.version.opporom")) {
        format!("ColorOS {v}")
    } else if let Some(v) = p.get("ro.vivo.os.build.display.id") {
        v.clone()
    } else if let Some(v) = p.get("ro.build.version.magic") {
        format!("MagicOS {v}")
    } else if let Some(v) = p.get("ro.build.version.emui") {
        v.clone()
    } else {
        get("ro.build.display.id")
    }
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
        self.map.keys().filter(|n| lists::pattern_matches(pattern, n)).cloned().collect()
    }
}
