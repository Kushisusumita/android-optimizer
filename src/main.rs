//! Android Optimizer - clean up, debloat and speed up any Android phone over ADB, without root.
//!
//! One responsibility per module:
//!   cli         - arguments, the menu, and each action's flow
//!   config      - constants and option types (no magic values scattered around)
//!   ui          - all terminal output and prompts
//!   adb         - locating and driving the `adb` executable
//!   platform_tools - downloading Google's adb when it is missing
//!   usb         - recognising a phone on USB before debugging is on
//!   device      - the phone: properties and installed packages, parsed once
//!   root        - optional read-only extras when the owner rooted the phone
//!   brands      - the brand registry: brand-exact instructions for every maker
//!   lists       - the package lists in `lists/*.txt`
//!   apps        - which packages may be removed, and why others are kept
//!   perms       - apps holding powerful permissions
//!   diagnostics - storage / RAM / battery / heat / stuck-key checks
//!   hardware    - SoC, CPU, GPU, current load and app start time
//!   score       - the health score and the before/after chart
//!   security    - heuristic malware audit
//!   adfinder    - the app behind pop-up ads
//!   tweaks      - reversible settings, ad-blocking DNS, cleanup and compilation
//!   screen      - tapping through settings adb cannot reach
//!   report      - the per-run report and undo records
//!   restore     - undo
//!   paths, util - project folders; small helpers

mod adb;
mod adfinder;
mod apps;
mod brands;
mod cli;
mod config;
mod device;
mod diagnostics;
mod hardware;
mod lists;
mod paths;
mod perms;
mod platform_tools;
mod report;
mod restore;
mod root;
mod score;
mod screen;
mod security;
mod tweaks;
mod ui;
mod usb;
mod util;

#[cfg(test)]
mod tests;

fn main() {
    std::process::exit(cli::run());
}
