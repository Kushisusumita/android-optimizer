//! The action menu, shown when no action was given on the command line.

use crate::config::{Action, Opts};
use crate::ui;

const MENU: &[(Action, &str)] = &[
    (Action::Optimize, "Optimize - remove ads and junk, block trackers, speed up (recommended)"),
    (Action::Max, "Maximum - also remove optional apps (you choose)"),
    (Action::SpeedUp, "Speed up only - clean caches and compile apps"),
    (Action::Security, "Security check - look for viruses and spyware"),
    (Action::AdSource, "Find the source of pop-up ads"),
    (Action::Check, "Check only - show what would change, change nothing"),
    (Action::Restore, "Restore - bring back removed apps and settings"),
];

pub fn choose_action(opts: &Opts) -> Action {
    if let Some(a) = opts.action {
        return a;
    }
    if opts.yes {
        return Action::Optimize;
    }
    ui::step("What should we do?");
    for (i, (_, text)) in MENU.iter().enumerate() {
        ui::say(&format!("{}. {text}", i + 1));
    }
    let n = ui::read_line("Your choice (Enter = 1)").parse::<usize>().ok().filter(|n| (1..=MENU.len()).contains(n));
    MENU[n.unwrap_or(1) - 1].0
}
