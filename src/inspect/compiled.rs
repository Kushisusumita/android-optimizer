//! How many of the apps a person opens are compiled ahead of time instead of interpreted on the fly.

use crate::phone::adb::Adb;
use crate::phone::packages;

/// Compiler states that mean "ahead-of-time compiled" (`verify`, `quicken` and friends do not).
const COMPILED: [&str; 3] = ["speed", "speed-profile", "everything"];

/// The first "[status=...]" in the "Dexopt state:" section of `dumpsys package <pkg>`.
pub fn compile_status(dump: &str) -> Option<String> {
    let section = &dump[dump.find("Dexopt state:")?..];
    let rest = &section[section.find("[status=")? + "[status=".len()..];
    Some(rest[..rest.find(']')?].to_string())
}

pub fn is_compiled(status: &str) -> bool {
    COMPILED.contains(&status)
}

/// (compiled, total) over the apps with a home-screen icon.
pub fn icon_apps(adb: &Adb) -> (usize, usize) {
    let apps = packages::launchable(adb);
    let compiled = apps
        .iter()
        // No `-m1` here: toybox grep then stops without printing the -A context lines.
        .filter_map(|p| compile_status(&adb.sh(&format!("dumpsys package {p} | grep -A4 'Dexopt state:'"))))
        .filter(|s| is_compiled(s))
        .count();
    (compiled, apps.len())
}
