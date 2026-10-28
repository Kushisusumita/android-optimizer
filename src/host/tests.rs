use super::report::{self, Report};
use crate::phone::packages::Packages;
use crate::test_support::{strings, temp_paths, xiaomi};
use std::fs;

#[test]
fn finds_apps_installed_since_the_last_run() {
    let paths = temp_paths("new-apps");
    let dir = paths.reports.join("2026-01-01_00-00-00_x_optimize");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("device.txt"), "device_id=TESTSERIAL\n").unwrap();
    fs::write(dir.join("packages-after.txt"), "com.old\n").unwrap();
    let pkgs = Packages::parse("package:com.old\npackage:com.new\npackage:com.sysnew\n", "package:com.sysnew\n", "");
    assert_eq!(report::new_since_last_run(&paths, "TESTSERIAL", &pkgs), strings(&["com.new"]));
    assert!(report::new_since_last_run(&paths, "OTHER", &pkgs).is_empty());
}

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
