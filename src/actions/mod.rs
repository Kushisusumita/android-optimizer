//! The user-facing actions, one per file. Each one wires the layers below it together; none of them
//! talks to adb or prints on its own.

mod optimize;
mod questions;
mod restore;
mod snapshot;
mod speedup;
mod steps;
#[cfg(test)]
mod tests;

use crate::config::{Action, Opts};
use crate::host::paths::Paths;
use crate::host::report::Report;
use crate::inspect::{adsource, security};
use crate::phone::Phone;

/// One run: the phone, the project folders, what the person asked for, and this run's report.
pub struct Session {
    pub phone: Phone,
    pub paths: Paths,
    pub opts: Opts,
    pub report: Report,
}

pub fn dispatch(action: Action, s: &Session) {
    if action == Action::Restore {
        restore::run(&s.phone, &s.paths, s.opts.yes);
        return;
    }
    let before = snapshot::take(s);
    s.report.write_packages("packages-before.txt", &before.pkgs);
    snapshot::show(s, &before);
    match action {
        Action::AdSource => adsource::find(&s.phone.adb, &before.pkgs),
        Action::SpeedUp => speedup::run(s, &before),
        _ => {
            security::run(&s.phone, &before.pkgs, &s.paths, s.report.path());
            if action != Action::Security {
                optimize::run(action, s, &before);
            }
        }
    }
}
