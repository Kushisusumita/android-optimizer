use super::cleanup::CompileState;
use super::{apps, cleanup, settings};

#[test]
fn recognises_refusals_and_appops_modes() {
    assert!(settings::refused("java.lang.SecurityException: Permission denial"));
    assert!(!settings::refused(""));
    assert_eq!(apps::parse_appops_mode("RUN_ANY_IN_BACKGROUND: ignore; time=+1d", "RUN_ANY_IN_BACKGROUND"), "ignore");
    assert_eq!(apps::parse_appops_mode("RUN_ANY_IN_BACKGROUND: allow", "RUN_ANY_IN_BACKGROUND"), "allow");
    assert_eq!(apps::parse_appops_mode("No operations.", "RUN_ANY_IN_BACKGROUND"), "default");
}

#[test]
fn display_size_never_squeezes_the_screen_below_320dp() {
    assert_eq!(settings::density_target(440, 1.10, 1080), 484);
    assert_eq!(settings::density_target(440, 1.30, 720), 360);
}

#[test]
fn parses_compile_progress_lines() {
    let ok = cleanup::parse_compile_line("12/47 OK com.pkg").unwrap();
    assert_eq!((ok.index, ok.total, ok.state, ok.package.as_str()), (12, 47, CompileState::Compiled, "com.pkg"));
    assert_eq!(cleanup::parse_compile_line("3/5 -- com.x").unwrap().state, CompileState::Failed);
    assert!(cleanup::parse_compile_line("1/2 FAIL com.x").is_none());
    assert!(cleanup::parse_compile_line("garbage").is_none());
}

#[test]
fn apps_android_already_compiled_are_left_alone() {
    let line = cleanup::parse_compile_line("4/23 == com.android.settings").unwrap();
    assert_eq!(line.state, CompileState::AlreadyCompiled);
    assert_eq!(line.package, "com.android.settings");
}

#[test]
fn only_user_added_battery_allowlist_entries_can_be_removed() {
    let list =
        "system-excidle,com.android.shell,2000\nsystem,com.google.android.gms,10123\nuser,com.example.chat,10234\n";
    assert!(apps::in_user_allowlist(list, "com.example.chat"));
    assert!(!apps::in_user_allowlist(list, "com.google.android.gms"), "system entries stay");
    assert!(!apps::in_user_allowlist(list, "com.example"), "whole names only");
}

#[test]
fn animations_change_only_from_the_default_never_from_the_owners_choice() {
    assert!(settings::animation_is_default("null"));
    assert!(settings::animation_is_default("1"));
    assert!(settings::animation_is_default("1.0"));
    assert!(!settings::animation_is_default("0.0"), "the owner turned animations off - leave it");
    assert!(!settings::animation_is_default("0.5"));
    assert!(!settings::animation_is_default("1.5"));
}
