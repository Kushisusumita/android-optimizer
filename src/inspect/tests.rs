use super::diagnostics::{self, Diag, Storage};
use super::{adsource, hardware, perms, score, security};
use crate::phone::packages::Packages;
use crate::test_support::strings;
use std::collections::HashSet;

#[test]
fn reads_storage() {
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

#[test]
fn a_phone_that_does_not_measure_wear_reports_no_battery_health() {
    // Samsung reports every "learned" value equal to the design capacity: nothing was measured.
    let stats = "  Estimated battery capacity: 5000 mAh\n  Last learned battery capacity: 5000 mAh\n  Min learned battery capacity: 5000 mAh\n  Max learned battery capacity: 5000 mAh\n";
    assert_eq!(diagnostics::capacity_from_stats(stats), None);
}

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
fn busiest_processes_come_from_the_last_sample_without_top_itself() {
    let top = "Tasks: 600 total\n800%cpu 3%user\n%CPU]NAME\n 5.0 surfaceflinger\n 0.0 init\n\
               Tasks: 600 total\n800%cpu 2%user\n%CPU]NAME\n10.0 top\n 3.0 com.android.systemui\n 0.0 adbd\n";
    assert_eq!(hardware::busiest(top), Some(vec![(3.0, "com.android.systemui".to_string())]));
    assert_eq!(hardware::busiest("%CPU NAME\n10.0 top\n 0.0 init\n"), Some(Vec::new()), "nothing busy");
    assert_eq!(hardware::busiest("top: unknown option -s"), None, "unreadable output is not 'nothing busy'");
}

#[test]
fn lists_device_admins_of_that_block_only() {
    let dump = "Current Device Policy Manager state:\n  Enabled Device Admins (User 0, provisioningState: 0):\n    com.evil.app/.AdminReceiver:\n      uid=10234\n    com.google.android.gms/.mdm.MdmReceiver:\n      uid=10123\n  Device Owner:\n    com.trap.app/.NotAnAdmin:\n";
    assert_eq!(security::parse_admins(dump), strings(&["com.evil.app", "com.google.android.gms"]));
}

#[test]
fn reads_permission_holders() {
    assert_eq!(perms::component_packages("com.a/.S:com.b/com.b.T"), strings(&["com.a", "com.b"]));
    assert!(perms::component_packages("null").is_empty());
    assert_eq!(perms::appops_packages("com.a\ncom.b\nNo operations.\n"), strings(&["com.a", "com.b"]));
}

#[test]
fn preinstalled_apps_from_the_lists_are_not_unknown_sources() {
    let pkgs = Packages::parse(
        "package:com.facebook.katana installer=null\npackage:com.unknown.app installer=null\n\
         package:com.side.app installer=com.google.android.packageinstaller\n",
        "",
        "",
    );
    let launchable: HashSet<String> =
        strings(&["com.facebook.katana", "com.unknown.app", "com.side.app"]).into_iter().collect();
    let listed: HashSet<String> = strings(&["com.facebook.katana"]).into_iter().collect();
    let found = security::origin_findings(&pkgs, &launchable, &listed).join("\n");
    assert!(found.contains("com.unknown.app - unknown source"));
    assert!(found.contains("com.side.app - installed from an APK file"));
    assert!(!found.contains("com.facebook.katana"));
}

#[test]
fn finds_the_app_on_screen_and_install_dates() {
    let resumed = "  mResumedActivity: ActivityRecord{c0ffee u0 com.evil.ads/.PopupActivity t42}";
    assert_eq!(adsource::first_component_package(resumed).as_deref(), Some("com.evil.ads"));
    assert_eq!(
        adsource::first_component_package("mCurrentFocus=Window{abc u0 com.foo/com.foo.Main}").as_deref(),
        Some("com.foo")
    );
    assert_eq!(adsource::install_date("    firstInstallTime=2025-03-14 10:22:01").as_deref(), Some("2025-03-14"));
    assert_eq!(adsource::install_date("nothing"), None);
}

// ---------------------------------------------------------------- app start speed and compile state

use super::{compiled, speed};

#[test]
fn finds_an_apps_launcher_activity_and_takes_the_median() {
    let resolved = "priority=0 preferredOrder=0 match=0x108000\ncom.android.settings/.Settings\n";
    assert_eq!(
        speed::parse_component(resolved),
        Some(("com.android.settings".to_string(), "com.android.settings/.Settings".to_string()))
    );
    assert_eq!(speed::parse_component("No activity found"), None);
    assert_eq!(speed::median(&[682, 737, 672, 680, 682]), Some(682));
    assert_eq!(speed::median(&[]), None);
}

#[test]
fn reads_an_apps_compile_state() {
    let dump = "Dexopt state:\n  [com.x]\n    path: /system/app/X.apk\n      arm64: [status=speed-profile] [reason=cmdline]\n  BgDexopt state:\n";
    assert_eq!(compiled::compile_status(dump).as_deref(), Some("speed-profile"));
    assert_eq!(compiled::compile_status("nothing here"), None);
    assert!(compiled::is_compiled("speed-profile"));
    assert!(compiled::is_compiled("speed"));
    assert!(!compiled::is_compiled("verify"));
}

// ---------------------------------------------------------------- settings other "optimizers" leave behind

use super::leftovers;

#[test]
fn finds_only_settings_changed_away_from_healthy() {
    let phone = |_ns: &str, key: &str| -> String {
        match key {
            "always_finish_activities" => "1".into(),
            "stay_on_while_plugged_in" => "3".into(),
            "adaptive_battery_management_enabled" => "null".into(),
            "app_standby_enabled" => "1".into(),
            "cached_apps_freezer" => "disabled".into(),
            _ => "null".into(),
        }
    };
    let keys: Vec<&str> = leftovers::found(phone).iter().map(|l| l.key).collect();
    assert_eq!(
        keys,
        vec!["always_finish_activities", "cached_apps_freezer"],
        "keeping the screen on while charging is a personal choice"
    );
}

#[test]
fn a_phone_with_default_settings_needs_no_repair() {
    assert!(leftovers::found(|_, _| "null".to_string()).is_empty());
    assert!(
        leftovers::found(|_, key| if key == "always_finish_activities" { "0".into() } else { "1".into() }).is_empty()
    );
}

// ---------------------------------------------------------------- calls

use super::calls::{self, Calls};

const NO_SIM_REGISTRY: &str = "    mServiceState={mVoiceRegState=1(OUT_OF_SERVICE), mDataRegState=1(OUT_OF_SERVICE), getRilVoiceRadioTechnology=0(Unknown), getRilDataRadioTechnology=0(Unknown), IsVoiceCallAvailable=false}\n";

#[test]
fn reads_the_voice_network_from_the_telephony_registry() {
    let none = calls::parse_voice(NO_SIM_REGISTRY);
    assert_eq!((none[0].in_service, none[0].radio), (false, 0));
    let lte = calls::parse_voice("mServiceState={mVoiceRegState=0(IN_SERVICE), getRilVoiceRadioTechnology=14(LTE)}");
    assert_eq!((lte[0].in_service, lte[0].radio), (true, 14));
}

#[test]
fn reads_what_the_carrier_offers() {
    let cfg = "  carrier_volte_available_bool = false\n  carrier_wfc_ims_available_bool = true\n";
    assert!(!calls::carrier_flag(cfg, "carrier_volte_available_bool"));
    assert!(calls::carrier_flag(cfg, "carrier_wfc_ims_available_bool"));
    assert!(calls::sim_present("READY,ABSENT"));
    assert!(!calls::sim_present("ABSENT,ABSENT"));
}

#[test]
fn tells_whether_calls_use_volte_or_fall_back() {
    let lte = calls::parse_voice("mServiceState={mVoiceRegState=0(IN_SERVICE), getRilVoiceRadioTechnology=14(LTE)}");
    let gsm = calls::parse_voice("mServiceState={mVoiceRegState=0(IN_SERVICE), getRilVoiceRadioTechnology=16(GSM)}");
    assert_eq!(calls::assess(false, &[], true), Calls::NoSim);
    assert_eq!(calls::assess(true, &calls::parse_voice(NO_SIM_REGISTRY), true), Calls::NoNetwork);
    assert_eq!(calls::assess(true, &lte, true), Calls::OverVolte);
    assert_eq!(calls::assess(true, &gsm, true), Calls::FallsBackVolteOff);
    assert_eq!(calls::assess(true, &gsm, false), Calls::FallsBackNoVolte);
}
