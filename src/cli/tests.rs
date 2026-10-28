use super::args::parse_args;
use crate::config::{self, Action, TextSize};
use crate::test_support::strings;

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
