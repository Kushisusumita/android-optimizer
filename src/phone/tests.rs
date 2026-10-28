use super::brands::{Brand, Props};
use super::info::{self, DeviceInfo};
use super::packages::{self, Packages};
use super::{root, usb};
use crate::test_support::{props, strings};

#[test]
fn glob_patterns_match_prefix_suffix_and_middle() {
    assert!(packages::name_matches("com.a.*", "com.a.b"));
    assert!(!packages::name_matches("com.a.*", "com.ab"));
    assert!(packages::name_matches("*.ads", "com.x.ads"));
    assert!(packages::name_matches("com.*.ads", "com.x.ads"));
    assert!(!packages::name_matches("ab*ba", "aba"));
    assert!(packages::name_matches("com.x", "com.x"));
    assert!(!packages::name_matches("com.x", "com.xy"));
}

#[test]
fn reads_getprop_and_builds_device_info() {
    let p = info::parse_getprop("[ro.product.model]: [SM-A155F]\n[ro.build.version.sdk]: [34]\nnoise\n");
    assert_eq!(p.get("ro.product.model").map(String::as_str), Some("SM-A155F"));

    let info = DeviceInfo::from_props(
        &props(&[
            ("ro.product.brand", "samsung"),
            ("ro.product.model", "SM-A155F"),
            ("ro.build.version.sdk", "34"),
            ("ro.build.version.oneui", "60100"),
            ("ro.soc.manufacturer", "Mediatek"),
            ("ro.soc.model", "MT6789"),
        ]),
        "R58X",
        false,
    );
    assert_eq!(info.brand, Brand::Samsung);
    assert_eq!(info.rom, "One UI 6.1");
    assert_eq!(info.name, "samsung SM-A155F");
    assert_eq!(info.device_id, "R58X");
    assert_eq!(info.sdk, 34);
    assert_eq!(info.soc, "Mediatek MT6789");
    assert_eq!(info.list_stems(), vec!["common", "google", "samsung"]);
}

#[test]
fn counts_accounts() {
    assert_eq!(info::accounts_count("Accounts: 2\n  Account {name=x}"), 2);
    assert_eq!(info::accounts_count("Accounts: 0"), 0);
    assert_eq!(info::accounts_count(""), 0);
}

#[test]
fn builds_the_package_set_from_pm_output() {
    let pkgs = Packages::parse(
        "package:com.a installer=com.android.vending\npackage:com.b installer=null\npackage:com.sys\n",
        "package:com.sys\n",
        "package:com.b\n",
    );
    assert_eq!(pkgs.len(), 3);
    assert_eq!(pkgs.get("com.a").unwrap().installer, "com.android.vending");
    assert_eq!(pkgs.get("com.b").unwrap().installer, "");
    assert!(!pkgs.get("com.b").unwrap().enabled);
    assert!(pkgs.get("com.sys").unwrap().system);
    assert_eq!(pkgs.user_app_count(), 2);
    assert_eq!(pkgs.matching("com.*").len(), 3);
    assert_eq!(pkgs.matching("com.a"), strings(&["com.a"]));
    assert!(pkgs.matching("com.zzz").is_empty());
    assert!(pkgs.is_third_party("com.a"));
    assert!(!pkgs.is_third_party("com.sys"));
    assert!(!pkgs.is_third_party("com.missing"));
}

#[test]
fn oem_names_need_the_full_prefix() {
    assert!(packages::is_oem("com.miui.home"));
    assert!(packages::is_oem("com.mi.health"));
    assert!(!packages::is_oem("com.military.game"));
    assert!(packages::is_oem("android.ext.services"));
}

#[test]
fn reads_screen_size() {
    assert_eq!(info::parse_wm_size("Physical size: 1080x2400\nOverride size: 720x1600"), Some((1080, 2400)));
    assert_eq!(info::parse_wm_size("garbage"), None);
}

#[test]
fn the_phone_title_does_not_repeat_the_model() {
    let samsung = DeviceInfo::from_props(
        &props(&[("ro.product.brand", "samsung"), ("ro.product.model", "SM-A127F")]),
        "x",
        false,
    );
    assert_eq!(samsung.title(), "samsung SM-A127F");
    let named = DeviceInfo::from_props(
        &props(&[
            ("ro.product.brand", "Redmi"),
            ("ro.product.model", "23124RA7EO"),
            ("ro.product.marketname", "Redmi Note 13"),
        ]),
        "x",
        false,
    );
    assert_eq!(named.title(), "Redmi Note 13 (23124RA7EO)");
}

#[test]
fn detects_brands_from_names_and_firmware() {
    let none = Props::new();
    assert_eq!(Brand::detect("Redmi", "Xiaomi", &none), Brand::Xiaomi);
    assert_eq!(Brand::detect("POCO", "", &none), Brand::Xiaomi);
    assert_eq!(Brand::detect("google", "Google", &props(&[("ro.miui.ui.version.name", "V14")])), Brand::Xiaomi);
    assert_eq!(Brand::detect("realme", "realme", &none), Brand::OppoRealmeOnePlus);
    assert_eq!(Brand::detect("Infinix", "INFINIX MOBILITY LIMITED", &none), Brand::Transsion);
    assert_eq!(Brand::detect("google", "Google", &none), Brand::Other);
}

#[test]
fn connect_steps_are_brand_exact() {
    let steps = Brand::Xiaomi.connect_steps().join("\n");
    assert!(steps.starts_with("1. "));
    assert!(steps.contains("OS version"));
    assert!(steps.contains("Security settings"));
    assert!(steps.contains("\n3. "));
    assert!(Brand::Samsung.connect_steps().join("\n").contains("Software information"));
}

#[test]
fn recognises_phones_on_usb_on_every_os() {
    let win = usb::parse_windows(
        "USB\\VID_2717&PID_FF48\\6D3B12AA|Redmi Note 13\r\nUSB\\VID_046D&PID_C52B\\5&2|USB Receiver\r\n",
    );
    assert_eq!(win, vec![(0x2717, "Redmi Note 13".to_string()), (0x046D, "USB Receiver".to_string())]);
    let phone = usb::pick_phone(win).unwrap();
    assert_eq!((phone.brand, phone.name.as_str()), (Brand::Xiaomi, "Redmi Note 13"));

    let linux = usb::parse_lsusb("Bus 001 Device 005: ID 2717:ff48 Xiaomi Inc. Mi/Redmi series (MTP)\n");
    assert_eq!(linux, vec![(0x2717, "Xiaomi Inc. Mi/Redmi series (MTP)".to_string())]);

    let mac = usb::parse_macos("USB:\n  USB 3.1 Bus:\n    Host Controller Driver: X\n      Galaxy A15:\n        Product ID: 0x6860\n        Vendor ID: 0x04e8  (Samsung)\n");
    assert_eq!(mac, vec![(0x04E8, "Galaxy A15".to_string())]);
    assert_eq!(Brand::from_usb_vendor(0x1234), None);
}

#[test]
fn picks_the_model_name_among_a_phones_usb_interfaces() {
    // Shape of a real Windows listing for one phone (IDs made up).
    let listing = "USB\\VID_04E8&PID_6860&MODEM\\6&1&0&0001|SAMSUNG Mobile USB Modem\n\
                   USB\\VID_04E8&PID_6860&ADB\\6&1&0&0003|SAMSUNG Android ADB Interface\n\
                   USB\\VID_04E8&PID_6860&MS_COMP_MTP&SAMSUNG_ANDROID\\6&1&0&0000|Galaxy A12\n\
                   USB\\VID_04E8&PID_6860\\TEST000|SAMSUNG Mobile USB Composite Device\n";
    let phone = usb::pick_phone(usb::parse_windows(listing)).unwrap();
    assert_eq!((phone.brand, phone.name.as_str()), (Brand::Samsung, "Galaxy A12"));
}

#[test]
fn prefers_a_model_name_over_a_generic_usb_name() {
    let phone = usb::pick_phone(vec![(0x04E8, "USB Composite Device".into()), (0x04E8, "Galaxy A15".into())]).unwrap();
    assert_eq!(phone.name, "Galaxy A15");
    assert!(usb::pick_phone(vec![(0x046D, "Mouse".into())]).is_none());
}

#[test]
fn notices_only_real_frequency_caps() {
    assert!(root::is_capped(2_000_000, 1_800_000));
    assert!(!root::is_capped(2_000_000, 1_950_000));
}

#[test]
fn reads_which_packages_have_a_home_screen_icon() {
    let icons = packages::parse_launchable("com.a/.Main\n  com.b/com.b.Launcher\n");
    assert_eq!(icons, ["com.a", "com.b"].iter().map(|s| s.to_string()).collect());
}

#[test]
fn reads_the_android_version_an_app_was_built_for() {
    assert_eq!(packages::parse_target_sdk("    versionCode=123 minSdk=21 targetSdk=25\n"), Some(25));
    assert_eq!(packages::parse_target_sdk("nothing"), None);
}
