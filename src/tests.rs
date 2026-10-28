//! Behaviour tests for everything that does not need a phone: parsers of adb output, the removal rules,
//! the score, the reports, and the command line. Each test fails if the code under it returned nothing.

use crate::apps::{self, Overrides};
use crate::brands::{Brand, Props};
use crate::cli::parse_args;
use crate::config::{self, Action, TextSize};
use crate::device::{self, DeviceInfo, Packages};
use crate::diagnostics::{self, Diag, Storage};
use crate::lists::{self, Section};
use crate::paths::Paths;
use crate::report::{self, Report};
use crate::{adfinder, hardware, perms, root, score, security, tweaks, ui, usb, util};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::PathBuf;

fn props(pairs: &[(&str, &str)]) -> Props {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// A fresh, empty project root in the temp folder (lists/ points at the real lists).
fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("android-optimizer-test-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn temp_paths(name: &str) -> Paths {
    Paths::locate(Some(temp_root(name).join("lists").to_str().unwrap())).unwrap()
}

fn repo_paths() -> Paths {
    Paths::locate(Some(concat!(env!("CARGO_MANIFEST_DIR"), "/lists"))).unwrap()
}

fn xiaomi(has_accounts: bool) -> DeviceInfo {
    let p = props(&[
        ("ro.product.brand", "Redmi"),
        ("ro.product.manufacturer", "Xiaomi"),
        ("ro.product.model", "TEST-MODEL"),
        ("ro.serialno", "TESTSERIAL"),
    ]);
    DeviceInfo::from_props(&p, "usb-serial", has_accounts)
}

// ---------------------------------------------------------------- command line

#[test]
fn parses_action_switches_lists_and_inline_values() {
    let o = parse_args(strings(&["optimize", "--yes", "--exclude", "a, b", "--text-size=2", "--font-scale", "1.2"]))
        .unwrap();
    assert_eq!(o.action, Some(Action::Optimize));
    assert!(o.yes);
    assert_eq!(o.exclude, strings(&["a", "b"]));
    assert_eq!(o.text_size, Some(TextSize::Larger));
    assert_eq!(o.font_scale, Some(1.2));
}

#[test]
fn rejects_bad_command_lines_with_the_reason() {
    assert!(parse_args(strings(&["--frobnicate"])).err().unwrap().contains("--frobnicate"));
    assert!(parse_args(strings(&["--serial"])).err().unwrap().contains("needs a value"));
    assert!(parse_args(strings(&["--font-scale", "9"])).err().unwrap().contains("scale"));
    assert!(parse_args(strings(&["dance"])).err().unwrap().contains("dance"));
}

#[test]
fn actions_and_text_sizes_accept_aliases() {
    assert_eq!(Action::parse("ads"), Some(Action::AdSource));
    assert_eq!(Action::parse("MAXIMUM"), Some(Action::Max));
    assert_eq!(Action::parse("nope"), None);
    assert_eq!(TextSize::parse("3"), Some(TextSize::Largest));
    assert_eq!(TextSize::Larger.scales(), config::TEXT_LARGER);
    assert_eq!(TextSize::Unchanged.scales(), (0.0, 0.0));
}

// ---------------------------------------------------------------- lists

#[test]
fn parses_list_sections_flags_and_descriptions() {
    let text = "# header\n[safe]\ncom.a.ads   # Ad thing @needs com.b.replacement\ncom.c.*  # Glob family @partner\n[max]\ncom.d.music # Music @media\n";
    let specs = lists::parse_list(text);
    assert_eq!(specs.len(), 3);
    assert_eq!(specs[0].pattern, "com.a.ads");
    assert_eq!(specs[0].section, Section::Safe);
    assert_eq!(specs[0].needs.as_deref(), Some("com.b.replacement"));
    assert_eq!(specs[0].desc, "Ad thing");
    assert!(specs[1].partner);
    assert_eq!(specs[1].desc, "Glob family");
    assert_eq!(specs[2].section, Section::Max);
    assert!(specs[2].media);
}

#[test]
fn plain_patterns_skip_headers_and_comments() {
    assert_eq!(lists::parse_patterns("[safe]\ncom.x # c\n\n# only a comment\ncom.y\n"), strings(&["com.x", "com.y"]));
}

#[test]
fn glob_patterns_match_prefix_suffix_and_middle() {
    assert!(lists::pattern_matches("com.a.*", "com.a.b"));
    assert!(!lists::pattern_matches("com.a.*", "com.ab"));
    assert!(lists::pattern_matches("*.ads", "com.x.ads"));
    assert!(lists::pattern_matches("com.*.ads", "com.x.ads"));
    assert!(!lists::pattern_matches("ab*ba", "aba"));
    assert!(lists::pattern_matches("com.x", "com.x"));
    assert!(!lists::pattern_matches("com.x", "com.xy"));
}

#[test]
fn keep_file_grows_without_duplicates() {
    let file = temp_root("keep").join("keep").join("phone.txt");
    lists::add_keep(&file, &strings(&["a", "b", "a"]));
    lists::add_keep(&file, &strings(&["b", "c"]));
    assert_eq!(lists::read_patterns(&file), strings(&["a", "b", "c"]));
}

// ---------------------------------------------------------------- device

#[test]
fn reads_getprop_and_builds_device_info() {
    let p = device::parse_getprop("[ro.product.model]: [SM-A155F]\n[ro.build.version.sdk]: [34]\nnoise\n");
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
    assert_eq!(device::accounts_count("Accounts: 2\n  Account {name=x}"), 2);
    assert_eq!(device::accounts_count("Accounts: 0"), 0);
    assert_eq!(device::accounts_count(""), 0);
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
    assert!(device::is_oem("com.miui.home"));
    assert!(device::is_oem("com.mi.health"));
    assert!(!device::is_oem("com.military.game"));
    assert!(device::is_oem("android.ext.services"));
}

#[test]
fn reads_screen_size() {
    assert_eq!(device::parse_wm_size("Physical size: 1080x2400\nOverride size: 720x1600"), Some((1080, 2400)));
    assert_eq!(device::parse_wm_size("garbage"), None);
}

// ---------------------------------------------------------------- brands and USB

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

// ---------------------------------------------------------------- removal rules

fn phone_packages() -> Packages {
    Packages::parse(
        "package:com.miui.msa.global installer=null\n\
         package:com.miui.videoplayer installer=null\n\
         package:com.android.settings installer=null\n\
         package:com.miui.analytics installer=null\n\
         package:com.xiaomi.ab installer=null\n\
         package:com.miui.systemAdSolution installer=null\n\
         package:com.android.browser installer=null\n\
         package:com.facebook.katana installer=com.android.vending\n\
         package:com.adups.fota installer=null\n",
        "package:com.miui.msa.global\npackage:com.miui.videoplayer\npackage:com.android.settings\n\
         package:com.miui.analytics\npackage:com.xiaomi.ab\npackage:com.miui.systemAdSolution\n\
         package:com.android.browser\npackage:com.adups.fota\n",
        "package:com.miui.analytics\n",
    )
}

#[test]
fn applies_every_safety_rule_when_choosing_removals() {
    let pkgs = phone_packages();
    let include = strings(&["com.android.settings"]);
    let exclude: HashSet<String> = strings(&["com.xiaomi.ab"]).into_iter().collect();
    let keep: HashSet<String> = strings(&["com.miui.systemAdSolution"]).into_iter().collect();
    let c = apps::candidates(
        &repo_paths(),
        &xiaomi(true),
        &pkgs,
        &Overrides { include: &include, exclude: &exclude, keep: &keep },
    );

    let names: Vec<&str> = c.items.iter().map(|i| i.name.as_str()).collect();
    assert!(names.contains(&"com.miui.msa.global"));
    assert!(names.contains(&"com.miui.videoplayer"));
    assert!(names.contains(&"com.adups.fota"));
    assert_eq!(names.len(), 3, "only the three removable packages: {names:?}");
    assert_eq!(c.junk_count(), 2, "Mi Video is optional (max), not junk");

    let notes = c.notes.join("\n");
    assert!(notes.contains("com.android.settings - protected"));
    assert!(notes.contains("com.miui.analytics - already disabled"));
    assert!(notes.contains("com.xiaomi.ab - excluded manually"));
    assert!(notes.contains("com.miui.systemAdSolution - kept by your decision"));
    assert!(notes.contains("com.android.browser - kept: no replacement (com.android.chrome)"));
    assert!(notes.contains("com.facebook.katana - installed by you (com.android.vending)"));
}

#[test]
fn without_an_account_a_play_partner_app_counts_as_preinstalled() {
    let none = HashSet::new();
    let c = apps::candidates(
        &repo_paths(),
        &xiaomi(false),
        &phone_packages(),
        &Overrides { include: &[], exclude: &none, keep: &none },
    );
    assert!(c.items.iter().any(|i| i.name == "com.facebook.katana"));
}

#[test]
fn finds_apps_installed_since_the_last_run() {
    let paths = temp_paths("new-apps");
    let dir = paths.reports.join("2026-01-01_00-00-00_x_optimize");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("device.txt"), "device_id=TESTSERIAL\n").unwrap();
    fs::write(dir.join("packages-after.txt"), "com.old\n").unwrap();
    let pkgs = Packages::parse("package:com.old\npackage:com.new\npackage:com.sysnew\n", "package:com.sysnew\n", "");
    assert_eq!(apps::new_since_last_run(&paths, "TESTSERIAL", &pkgs), strings(&["com.new"]));
    assert!(apps::new_since_last_run(&paths, "OTHER", &pkgs).is_empty());
}

// ---------------------------------------------------------------- reports

#[test]
fn reports_remember_the_device_and_the_first_original_value() {
    let paths = temp_paths("reports");
    let info = xiaomi(true);
    let r = Report::create(&paths, &info, "optimize");
    assert!(fs::read_to_string(r.path().join("device.txt")).unwrap().contains("device_id=TESTSERIAL"));
    assert_eq!(report::device_reports(&paths, "TESTSERIAL").len(), 1);
    assert!(report::device_reports(&paths, "OTHER").is_empty());
    assert_eq!(report::last_fingerprint(&paths, "TESTSERIAL").as_deref(), Some(""));

    let (old, new) = (paths.reports.join("2026-01-01_a"), paths.reports.join("2026-02-01_b"));
    for (dir, value) in [(&old, "original"), (&new, "changed")] {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("tweaks.tsv"), format!("global\tkey\t{value}\n")).unwrap();
    }
    fs::write(new.join("tweaks.tsv"), "global\tkey\tchanged\nsystem\tfont_scale\t1.0\n").unwrap();
    let first = report::read_first_values(&[old, new], "tweaks.tsv", 2);
    assert_eq!(first, vec![("global\tkey".into(), "original".into()), ("system\tfont_scale".into(), "1.0".into())]);
}

// ---------------------------------------------------------------- diagnostics

#[test]
fn reads_numbers_and_storage() {
    assert_eq!(diagnostics::num_after("MemTotal:        3800000 kB", "MemTotal:"), Some(3_800_000));
    assert_eq!(diagnostics::num_after("nothing", "MemTotal:"), None);
    let s = diagnostics::parse_df(
        "Filesystem 1K-blocks Used Available Use% Mounted on\n/dev/block/dm-47 114756552 98123456 16502424 86% /data\n",
    )
    .unwrap();
    assert_eq!((s.total_mb, s.free_mb, s.pct), (112_066, 16_115, 86));
}

fn volume_presses(count: usize, step_ms: usize) -> String {
    (0..count)
        .map(|i| {
            let ms = i * step_ms;
            format!(
                "10-28 10:00:{:02}:{:03} adjustSuggestedStreamVolume(stream:3 dir:ADJUST_RAISE flags:0x1) from android/android\n",
                ms / 1000,
                ms % 1000
            )
        })
        .collect()
}

#[test]
fn detects_a_stuck_volume_key_only_when_presses_are_fast_and_recent() {
    let now = diagnostics::parse_hms("10:00:05").unwrap();
    assert_eq!(diagnostics::stuck_key(&volume_presses(30, 50), now), Some("Volume Up"));
    assert_eq!(diagnostics::stuck_key(&volume_presses(30, 200), now), None, "too slow: a person pressing");
    assert_eq!(diagnostics::stuck_key(&volume_presses(29, 50), now), None, "too few");
    let later = diagnostics::parse_hms("10:05:00").unwrap();
    assert_eq!(diagnostics::stuck_key(&volume_presses(30, 50), later), None, "long ago");
}

#[test]
fn reads_battery_wear_from_android_stats() {
    let stats = "  Estimated battery capacity: 5000 mAh\n  Max learned battery capacity: 4100 mAh\n";
    assert_eq!(diagnostics::capacity_from_stats(stats), Some((82, "4100 of 5000 mAh".to_string())));
    assert_eq!(diagnostics::capacity_from_stats("  Estimated battery capacity: 5000 mAh\n"), None);
}

// ---------------------------------------------------------------- score

fn healthy() -> Diag {
    Diag {
        ram_total_mb: 4000,
        ram_free_mb: 2000,
        storage: Some(Storage { total_mb: 64_000, free_mb: 38_000, pct: 40 }),
        battery_health: 2,
        battery_capacity_pct: 95,
        ..Diag::default()
    }
}

#[test]
fn a_healthy_phone_scores_100_and_problems_cost_points() {
    assert_eq!(score::score(&healthy(), 0), 100);
    let full = Diag { storage: Some(Storage { total_mb: 64_000, free_mb: 3_000, pct: 95 }), ..healthy() };
    assert_eq!(score::score(&full, 0), 70);
    assert_eq!(score::score(&healthy(), 1000), 80, "junk penalty is capped");
    let stuck = Diag { stuck_volume_key: Some("Volume Up"), ..healthy() };
    assert_eq!(score::score(&stuck, 0), 85);
}

#[test]
fn score_labels_have_clear_boundaries() {
    assert_eq!(score::label(90), "excellent");
    assert_eq!(score::label(89), "good");
    assert_eq!(score::label(54), "poor");
    assert_eq!(score::label(34), "bad");
}

// ---------------------------------------------------------------- hardware and root

#[test]
fn names_the_gpu_and_cpu_clusters() {
    assert_eq!(
        hardware::parse_gpu("GLES: Qualcomm, Adreno (TM) 610, OpenGL ES 3.2 V@0502").as_deref(),
        Some("Qualcomm Adreno (TM) 610")
    );
    assert_eq!(hardware::parse_gpu("GLES: ARM, Mali-G57 MC2, OpenGL ES 3.2").as_deref(), Some("ARM Mali-G57 MC2"));
    assert_eq!(hardware::parse_gpu("nothing"), None);
    let freqs = "2000000\n2000000\n2000000\n2000000\n2000000\n2000000\n2200000\n2200000\n";
    assert_eq!(hardware::describe_clusters(freqs).as_deref(), Some("6 x 2.0 GHz + 2 x 2.2 GHz"));
    assert_eq!(hardware::describe_clusters(""), None);
}

#[test]
fn computes_cpu_load_between_two_samples() {
    let first = hardware::cpu_times("cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 1 1 1 1\n").unwrap();
    assert_eq!(first, (850, 1000));
    assert_eq!(hardware::busy_percent(first, (900, 1200)), Some(75));
    assert_eq!(hardware::busy_percent(first, first), None);
}

#[test]
fn notices_only_real_frequency_caps() {
    assert!(root::is_capped(2_000_000, 1_800_000));
    assert!(!root::is_capped(2_000_000, 1_950_000));
    assert_eq!(root::leading_number(" 1800000 kHz"), Some(1_800_000));
    assert_eq!(root::leading_number("abc"), None);
}

// ---------------------------------------------------------------- security and ads

#[test]
fn lists_device_admins_of_that_block_only() {
    let dump = "Current Device Policy Manager state:\n  Enabled Device Admins (User 0, provisioningState: 0):\n    com.evil.app/.AdminReceiver:\n      uid=10234\n    com.google.android.gms/.mdm.MdmReceiver:\n      uid=10123\n  Device Owner:\n    com.trap.app/.NotAnAdmin:\n";
    assert_eq!(security::parse_admins(dump), strings(&["com.evil.app", "com.google.android.gms"]));
}

#[test]
fn reads_launcher_icons_and_permission_holders() {
    let icons = security::parse_launchable("com.a/.Main\n  com.b/com.b.Launcher\n");
    assert_eq!(icons, ["com.a", "com.b"].iter().map(|s| s.to_string()).collect());
    assert_eq!(perms::component_packages("com.a/.S:com.b/com.b.T"), strings(&["com.a", "com.b"]));
    assert!(perms::component_packages("null").is_empty());
    assert_eq!(perms::appops_packages("com.a\ncom.b\nNo operations.\n"), strings(&["com.a", "com.b"]));
}

#[test]
fn finds_the_app_on_screen_and_install_dates() {
    let resumed = "  mResumedActivity: ActivityRecord{c0ffee u0 com.evil.ads/.PopupActivity t42}";
    assert_eq!(adfinder::first_component_package(resumed).as_deref(), Some("com.evil.ads"));
    assert_eq!(
        adfinder::first_component_package("mCurrentFocus=Window{abc u0 com.foo/com.foo.Main}").as_deref(),
        Some("com.foo")
    );
    assert_eq!(adfinder::install_date("    firstInstallTime=2025-03-14 10:22:01").as_deref(), Some("2025-03-14"));
    assert_eq!(adfinder::install_date("nothing"), None);
}

// ---------------------------------------------------------------- settings and compilation

#[test]
fn recognises_refusals_and_appops_modes() {
    assert!(tweaks::refused("java.lang.SecurityException: Permission denial"));
    assert!(!tweaks::refused(""));
    assert_eq!(tweaks::parse_appops_mode("RUN_ANY_IN_BACKGROUND: ignore; time=+1d", "RUN_ANY_IN_BACKGROUND"), "ignore");
    assert_eq!(tweaks::parse_appops_mode("RUN_ANY_IN_BACKGROUND: allow", "RUN_ANY_IN_BACKGROUND"), "allow");
    assert_eq!(tweaks::parse_appops_mode("No operations.", "RUN_ANY_IN_BACKGROUND"), "default");
}

#[test]
fn display_size_never_squeezes_the_screen_below_320dp() {
    assert_eq!(tweaks::density_target(440, 1.10, 1080), 484);
    assert_eq!(tweaks::density_target(440, 1.30, 720), 360);
}

#[test]
fn parses_compile_progress_lines() {
    let ok = tweaks::parse_compile_line("12/47 OK com.pkg").unwrap();
    assert_eq!((ok.index, ok.total, ok.ok, ok.package.as_str()), (12, 47, true, "com.pkg"));
    assert!(!tweaks::parse_compile_line("3/5 -- com.x").unwrap().ok);
    assert!(tweaks::parse_compile_line("1/2 FAIL com.x").is_none());
    assert!(tweaks::parse_compile_line("garbage").is_none());
}

// ---------------------------------------------------------------- small helpers

#[test]
fn dates_and_names() {
    assert_eq!(util::days_from_civil(1970, 1, 1), 0);
    assert_eq!(util::days_from_civil(2000, 3, 1), 11_017);
    assert_eq!(util::parse_ymd_to_days("2024-03-01").unwrap() - util::parse_ymd_to_days("2024-02-28").unwrap(), 2);
    assert_eq!(util::slug("Redmi Note 13 Pro+"), "Redmi-Note-13-Pro");
    let stamp = util::now_utc_compact();
    assert_eq!((stamp.len(), stamp.as_bytes()[10]), (19, b'_'));
}

#[test]
fn number_picking_and_bars() {
    assert_eq!(ui::parse_numbers("1, 3-4 9 0", 5), BTreeSet::from([1, 3, 4]));
    assert_eq!(ui::split_by_numbers(&["a", "b", "c"], &BTreeSet::from([2])), (vec!["b"], vec!["a", "c"]));
    assert_eq!(ui::bar(0.5), format!("[{}{}]", "#".repeat(12), "-".repeat(12)));
    assert_eq!(ui::format_mb(2048), "2.0 GB");
}
