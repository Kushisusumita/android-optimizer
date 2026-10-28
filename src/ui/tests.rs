use super::chart::bar;
use super::*;
use std::collections::BTreeSet;

#[test]
fn number_picking() {
    assert_eq!(parse_numbers("1, 3-4 9 0", 5), BTreeSet::from([1, 3, 4]));
    assert_eq!(split_by_numbers(&["a", "b", "c"], &BTreeSet::from([2])), (vec!["b"], vec!["a", "c"]));
}

#[test]
fn bars_and_sizes() {
    assert_eq!(bar(0.5), format!("[{}{}]", "#".repeat(12), "-".repeat(12)));
    assert_eq!(bar(7.0), format!("[{}]", "#".repeat(24)), "clamped");
    assert_eq!(format_mb(2048), "2.0 GB");
    assert_eq!(format_mb(512), "512 MB");
}
