use super::questions::offered_changes;
use crate::config::Opts;
use crate::phone::info::DeviceInfo;
use crate::test_support::{props, xiaomi};
use crate::tune::{Change, Plan};

fn samsung_without_account() -> DeviceInfo {
    let p = props(&[("ro.product.brand", "samsung"), ("ro.product.model", "SM-A127F"), ("ro.build.version.sdk", "33")]);
    DeviceInfo::from_props(&p, "x", false)
}

#[test]
fn every_automatic_change_is_offered_only_where_it_applies() {
    let samsung = offered_changes(&samsung_without_account(), &Opts::default(), true);
    assert!(samsung.contains(&Change::Animations));
    assert!(samsung.contains(&Change::AdBlockDns));
    assert!(!samsung.contains(&Change::XiaomiRecommendations), "Xiaomi-only");
    assert!(!samsung.contains(&Change::AdPersonalization), "needs a Google account");
    let mi = offered_changes(&xiaomi(true), &Opts::default(), true);
    assert!(mi.contains(&Change::XiaomiRecommendations));
    assert!(mi.contains(&Change::AdPersonalization));
}

#[test]
fn without_settings_access_only_cleanup_and_compilation_are_offered() {
    let offered = offered_changes(&samsung_without_account(), &Opts::default(), false);
    assert_eq!(offered, vec![Change::CleanCaches, Change::Compile]);
}

#[test]
fn command_line_switches_take_their_change_off_the_list() {
    let opts = Opts { no_dns: true, no_compile: true, ..Opts::default() };
    let offered = offered_changes(&samsung_without_account(), &opts, true);
    assert!(!offered.contains(&Change::AdBlockDns));
    assert!(!offered.contains(&Change::Compile));
    assert!(offered.contains(&Change::CleanCaches));
}

#[test]
fn a_plan_does_only_what_was_chosen() {
    let plan = Plan { changes: vec![Change::Animations], ..Plan::default() };
    assert!(plan.wants(Change::Animations));
    assert!(!plan.wants(Change::AdBlockDns));
}

// ---------------------------------------------------------------- results in words

use super::results::{compare, rows, Direction, RunFacts};
use crate::ui::chart::{Measure, Row, Verdict};

fn measure(
    score: i32,
    settings_ms: i64,
    ram_free_mb: i64,
    storage_free_mb: i64,
    packages: usize,
    compiled: usize,
) -> Measure {
    Measure {
        score,
        score_label: "excellent",
        launches: vec![("Settings".to_string(), settings_ms)],
        ram_free_mb,
        ram_total_mb: 3686,
        storage_free_mb,
        storage_total_mb: 52_634,
        packages,
        compiled_icon_apps: (compiled, 23),
    }
}

fn row<'a>(rows: &'a [Row], title: &str) -> &'a Row {
    rows.iter().find(|r| r.title.starts_with(title)).unwrap_or_else(|| panic!("no row {title}"))
}

#[test]
fn verdicts_respect_the_direction_and_measurement_noise() {
    assert_eq!(compare(Direction::LowerIsBetter, 682.0, 613.0, 34.0), Verdict::Better);
    assert_eq!(compare(Direction::LowerIsBetter, 660.0, 695.0, 33.0), Verdict::Worse);
    assert_eq!(compare(Direction::LowerIsBetter, 660.0, 680.0, 33.0), Verdict::Same);
    assert_eq!(compare(Direction::HigherIsBetter, 95.0, 97.0, 1.0), Verdict::Better);
    assert_eq!(compare(Direction::HigherIsBetter, 95.0, 95.0, 1.0), Verdict::Same);
}

#[test]
fn every_result_row_says_in_words_what_changed_and_why() {
    let before = measure(95, 682, 1536, 43_520, 356, 4);
    let after = measure(97, 560, 2048, 41_677, 346, 23);
    let r = rows(&before, &after, &RunFacts { removed: 10, compiled: true });
    assert_eq!(row(&r, "Health score").verdict, Verdict::Better);
    assert!(row(&r, "Health score").reason.contains("10 junk apps removed"));
    assert_eq!(row(&r, "Settings").verdict, Verdict::Better);
    assert_eq!(row(&r, "Free RAM").verdict, Verdict::Better);
    assert_eq!(row(&r, "Free storage").verdict, Verdict::Expected);
    assert!(row(&r, "Free storage").reason.contains("compiled"));
    assert_eq!(row(&r, "Apps with an icon compiled").verdict, Verdict::Better);
    assert_eq!(row(&r, "Apps in the system").verdict, Verdict::Better);
}

#[test]
fn storage_lost_without_compilation_is_reported_as_worse() {
    let before = measure(95, 682, 1536, 43_520, 356, 4);
    let after = measure(95, 682, 1536, 40_000, 356, 4);
    let r = rows(&before, &after, &RunFacts { removed: 0, compiled: false });
    assert_eq!(row(&r, "Free storage").verdict, Verdict::Worse);
}
