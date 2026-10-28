//! How a person drives the tool from a terminal: arguments, the action menu, connecting to the phone.
//! Then the chosen action runs (see `actions`).

mod args;
mod connect;
mod menu;
#[cfg(test)]
mod tests;

use crate::actions::{self, Session};
use crate::config::{self, Opts};
use crate::host::paths::Paths;
use crate::host::report::Report;
use crate::ui;

const NO_LISTS: &str = "The lists/ folder was not found. Run from the project folder or pass --lists <folder>.";

/// The program: returns the process exit code (0 done, 1 failed, 2 bad command line).
pub fn run() -> i32 {
    let opts = match args::parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            ui::fail(&e);
            ui::say("Run with --help to see the options.");
            return 2;
        }
    };
    if opts.help {
        args::usage();
        return 0;
    }
    if opts.version {
        println!("{} {}", config::APP_NAME, config::APP_VERSION);
        return 0;
    }
    ui::banner();
    match start(opts) {
        Ok(()) => 0,
        Err(e) => {
            ui::fail(&e);
            1
        }
    }
}

fn start(opts: Opts) -> Result<(), String> {
    let paths = Paths::locate(opts.lists_dir.as_deref()).ok_or(NO_LISTS)?;
    let adb = connect::locate_adb(&paths, &opts)?;
    adb.start_server();
    let action = menu::choose_action(&opts);
    let phone = connect::connect(adb, &opts)?;
    connect::introduce(&paths, &phone.info);
    let report = Report::create(&paths, &phone.info, action.tag());
    actions::dispatch(action, &Session { phone, paths, opts, report });
    Ok(())
}
