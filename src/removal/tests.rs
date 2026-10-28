use super::candidates::{self, Overrides};
use super::lists::{self, Section};
use crate::phone::packages::Packages;
use crate::test_support::{repo_paths, strings, temp_root, xiaomi};
use std::collections::HashSet;

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
fn keep_file_grows_without_duplicates() {
    let file = temp_root("keep").join("keep").join("phone.txt");
    lists::add_keep(&file, &strings(&["a", "b", "a"]));
    lists::add_keep(&file, &strings(&["b", "c"]));
    assert_eq!(lists::read_patterns(&file), strings(&["a", "b", "c"]));
}

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
    let c = candidates::candidates(
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
    let c = candidates::candidates(
        &repo_paths(),
        &xiaomi(false),
        &phone_packages(),
        &Overrides { include: &[], exclude: &none, keep: &none },
    );
    assert!(c.items.iter().any(|i| i.name == "com.facebook.katana"));
}
