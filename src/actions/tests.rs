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
