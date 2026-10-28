//! Android Optimizer - clean up, debloat and speed up any Android phone over ADB, without root.
//!
//! Layers, top to bottom. Each one uses only the layers below it:
//!
//!   cli      how a person drives it: arguments, the action menu, connecting to the phone
//!   actions  one user-facing action per file: optimize / max / check, speed up, restore
//!   tune     what it changes on the phone, always reversibly: settings, ad-blocking DNS, apps, cleanup
//!   inspect  what it reads from the phone, changing nothing: health, hardware, score, security, ad source
//!   removal  which packages may go: the lists and the safety rules applied to them
//!   host     the computer: project folders, per-run reports, downloading adb
//!   phone    talking to the phone: adb, properties, packages, screen, USB, root - raw output parsed once
//!
//! Used by every layer: `ui` (all output, questions, colours, charts), `config` (constants and the
//! parsed options), `util` (number parsing, dates).

mod actions;
mod cli;
mod config;
mod host;
mod inspect;
mod phone;
mod removal;
mod tune;
mod ui;
mod util;

#[cfg(test)]
mod test_support;

fn main() {
    let code = cli::run();
    ui::keep_window_open_if_needed();
    std::process::exit(code);
}
