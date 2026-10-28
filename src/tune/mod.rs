//! What the tool changes on the phone - always reversibly, with every original value recorded for Restore.

pub mod adblock;
pub mod apps;
pub mod cleanup;
pub mod screen_flows;
pub mod settings;
#[cfg(test)]
mod tests;

use crate::host::paths::Paths;
use crate::phone::info::DeviceInfo;
use crate::ui;
use settings::Tweaks;

/// The settings the person chose beyond the always-on optimizations. A scale of 0.0 means "leave as is".
pub struct Plan {
    pub font: f64,
    pub display: f64,
    pub background: Vec<String>,
    pub mute: Vec<String>,
}

/// Smoothness, battery, text size, app limits and ad personalization, in that order.
pub fn apply(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan, paths: &Paths) {
    ui::step("Smoothness and battery settings");
    settings::apply_speed_settings(tw, info);
    settings::apply_text_size(tw, plan);
    apps::apply_background(tw, info, plan, paths);
    apps::apply_mute(tw, info, plan);
    if info.has_accounts && screen_flows::set_ad_personalization_off(tw.adb) {
        ui::ok("Google ad personalization off (ads no longer follow your interests)");
    }
}
