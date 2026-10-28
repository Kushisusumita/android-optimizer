//! The brand registry. One place that knows how to recognise a phone's maker and give brand-exact
//! instructions - so support is not "Xiaomi-only" but covers every model family.

use std::collections::HashMap;

pub type Props = HashMap<String, String>;

/// USB vendor IDs of phone makers (as registered with USB-IF).
const USB_VENDORS: &[(u16, Brand)] = &[
    (0x2717, Brand::Xiaomi),
    (0x04E8, Brand::Samsung),
    (0x22D9, Brand::OppoRealmeOnePlus),
    (0x2A70, Brand::OppoRealmeOnePlus),
    (0x2D95, Brand::Vivo),
    (0x12D1, Brand::HuaweiHonor),
    (0x339B, Brand::HuaweiHonor),
    (0x22B8, Brand::Motorola),
    (0x17EF, Brand::Motorola),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Brand {
    Xiaomi,
    Samsung,
    OppoRealmeOnePlus,
    Vivo,
    HuaweiHonor,
    Transsion,
    Motorola,
    Other,
}

impl Brand {
    /// Detect the brand from brand/manufacturer strings and firmware properties.
    pub fn detect(brand: &str, manufacturer: &str, props: &Props) -> Brand {
        let s = format!("{brand} {manufacturer}").to_ascii_lowercase();
        let has = |k: &str| props.contains_key(k);
        if s.contains("xiaomi")
            || s.contains("redmi")
            || s.contains("poco")
            || s.contains("blackshark")
            || has("ro.miui.ui.version.name")
            || has("ro.mi.os.version.name")
        {
            Brand::Xiaomi
        } else if s.contains("samsung") {
            Brand::Samsung
        } else if s.contains("oppo")
            || s.contains("realme")
            || s.contains("oneplus")
            || has("ro.build.version.oplusrom")
        {
            Brand::OppoRealmeOnePlus
        } else if s.contains("vivo") || s.contains("iqoo") {
            Brand::Vivo
        } else if s.contains("huawei") || s.contains("honor") {
            Brand::HuaweiHonor
        } else if s.contains("tecno") || s.contains("infinix") || s.contains("itel") {
            Brand::Transsion
        } else if s.contains("motorola") || s.contains("lenovo") {
            Brand::Motorola
        } else {
            Brand::Other
        }
    }

    /// The brand's own list file stem in `lists/`, if any (common/google/malware apply to every brand).
    pub fn list_stem(self) -> Option<&'static str> {
        match self {
            Brand::Xiaomi => Some("xiaomi"),
            Brand::Samsung => Some("samsung"),
            Brand::OppoRealmeOnePlus => Some("oppo-realme-oneplus"),
            Brand::Vivo => Some("vivo"),
            Brand::HuaweiHonor => Some("huawei-honor"),
            Brand::Transsion => Some("transsion"),
            Brand::Motorola => Some("motorola"),
            Brand::Other => None,
        }
    }

    /// The maker from a USB vendor ID - known before USB debugging is on (charging/file-transfer mode).
    pub fn from_usb_vendor(vendor_id: u16) -> Option<Brand> {
        USB_VENDORS.iter().find(|(id, _)| *id == vendor_id).map(|(_, b)| *b)
    }

    pub fn title(self) -> &'static str {
        match self {
            Brand::Xiaomi => "Xiaomi / Redmi / POCO",
            Brand::Samsung => "Samsung",
            Brand::OppoRealmeOnePlus => "OPPO / realme / OnePlus",
            Brand::Vivo => "vivo / iQOO",
            Brand::HuaweiHonor => "Huawei / Honor",
            Brand::Transsion => "Tecno / Infinix / itel",
            Brand::Motorola => "Motorola / Lenovo",
            Brand::Other => "Android",
        }
    }

    /// Every step from "Developer options are hidden" to "the computer sees the phone", for this brand.
    pub fn connect_steps(self) -> Vec<String> {
        let mut steps = vec![format!("1. Turn on Developer options: {}.", self.build_number_hint())];
        steps.extend(self.setup_help().iter().enumerate().map(|(i, l)| {
            if i == 0 {
                format!("2. {l}")
            } else {
                format!("   {l}")
            }
        }));
        steps
            .push("3. Plug in a data cable; on the phone tap Allow (tick \"Always allow from this computer\").".into());
        steps
    }

    /// Where the VoLTE switch usually is (names vary a little between system versions).
    pub fn volte_setting(self) -> &'static str {
        match self {
            Brand::Samsung => "Settings -> Connections -> Mobile networks -> VoLTE calls",
            Brand::Xiaomi => "Settings -> SIM cards & mobile networks -> your SIM -> VoLTE",
            Brand::OppoRealmeOnePlus => "Settings -> Mobile network -> your SIM -> VoLTE",
            Brand::Vivo => "Settings -> Mobile network -> your SIM -> VoLTE",
            Brand::HuaweiHonor => "Settings -> Mobile network -> Mobile data -> VoLTE calls",
            _ => "Settings -> Network & internet -> SIMs -> your SIM -> VoLTE / 4G calling",
        }
    }

    /// Where to find the build number (to enable Developer options).
    pub fn build_number_hint(self) -> &'static str {
        match self {
            Brand::Xiaomi => "Settings -> About phone -> tap \"OS version\" / \"MIUI version\" 7 times",
            Brand::Samsung => "Settings -> About phone -> Software information -> tap \"Build number\" 7 times",
            Brand::Vivo => "Settings -> About phone -> tap \"Software version\" 7 times",
            _ => "Settings -> About phone -> tap \"Build number\" 7 times",
        }
    }

    /// Brand-exact setup steps, shown once the model is known (e.g. when the phone blocks setting changes).
    pub fn setup_help(self) -> Vec<&'static str> {
        match self {
            Brand::Xiaomi => vec![
                "Xiaomi/Redmi/POCO: Settings -> Additional settings -> Developer options.",
                "To change settings also turn on \"USB debugging (Security settings)\":",
                "enable \"Install via USB\" first; it needs a SIM card, a Xiaomi account and mobile data.",
            ],
            Brand::OppoRealmeOnePlus => vec![
                "Oppo/Realme/OnePlus: Settings -> Additional settings -> Developer options.",
                "Turn on \"USB debugging\" and, if present, \"Disable permission monitoring\".",
            ],
            Brand::Vivo => vec![
                "vivo/iQOO: Settings -> System management (or More settings) -> Developer options.",
                "Turn on \"USB debugging\".",
            ],
            Brand::Samsung => vec!["Samsung: Settings -> Developer options (at the very bottom) -> USB debugging."],
            Brand::HuaweiHonor => {
                vec!["Huawei/Honor: Settings -> System & updates -> Developer options -> USB debugging."]
            }
            Brand::Transsion => vec!["Tecno/Infinix/itel: Settings -> System -> Developer options -> USB debugging."],
            Brand::Motorola => vec!["Motorola/Lenovo: Settings -> System -> Developer options -> USB debugging."],
            Brand::Other => vec!["Settings -> Developer options -> USB debugging."],
        }
    }
}
