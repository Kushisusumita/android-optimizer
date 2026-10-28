//! What the phone is: its properties from `getprop`, parsed once into `DeviceInfo`.

use super::adb::Adb;
use super::brands::{Brand, Props};

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

    /// "Redmi Note 13 (23124RA7EO)", or just "samsung SM-A127F" when the name already holds the model.
    pub fn title(&self) -> String {
        if self.name.contains(&self.model) {
            self.name.clone()
        } else {
            format!("{} ({})", self.name, self.model)
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
