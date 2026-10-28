//! What the tool changes on the phone - always reversibly, with every original value recorded for Restore.
//! Nothing here runs unless the person chose it (see `actions::questions`).

pub mod adblock;
pub mod apps;
pub mod cleanup;
pub mod screen_flows;
pub mod settings;
#[cfg(test)]
mod tests;

use crate::host::paths::Paths;
use crate::phone::brands::Brand;
use crate::phone::info::DeviceInfo;
use crate::ui;
use settings::Tweaks;

/// Android 9 brought Private DNS; Android 7 the speed compiler.
const PRIVATE_DNS_SDK: i32 = 28;
const COMPILE_SDK: i32 = 24;

/// Every change a run can make on its own, each one offered for confirmation first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Animations,
    Scanning,
    MobileData,
    XiaomiRecommendations,
    AdPersonalization,
    AdBlockDns,
    CleanCaches,
    Compile,
}

impl Change {
    pub const ALL: [Change; 8] = [
        Change::Animations,
        Change::Scanning,
        Change::MobileData,
        Change::XiaomiRecommendations,
        Change::AdPersonalization,
        Change::AdBlockDns,
        Change::CleanCaches,
        Change::Compile,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Change::Animations => "Faster animations (2x) - the interface feels snappier",
            Change::Scanning => "No constant Wi-Fi/Bluetooth scanning in the background (location still works)",
            Change::MobileData => "Do not keep mobile data on while on Wi-Fi - saves battery",
            Change::XiaomiRecommendations => "No app recommendations on the Xiaomi home screen",
            Change::AdPersonalization => "Turn off Google ad personalization",
            Change::AdBlockDns => "Block ads and trackers in all apps and browsers (Private DNS)",
            Change::CleanCaches => "Clear app caches and run storage maintenance",
            Change::Compile => "Compile apps for speed (can take up to an hour)",
        }
    }

    /// Changes that go through `settings`/`input`, which some brands lock (Xiaomi without its switch).
    pub fn needs_settings_access(self) -> bool {
        !matches!(self, Change::CleanCaches | Change::Compile)
    }

    pub fn applies_to(self, info: &DeviceInfo) -> bool {
        match self {
            Change::XiaomiRecommendations => info.brand == Brand::Xiaomi,
            Change::AdPersonalization => info.has_accounts,
            Change::AdBlockDns => info.sdk >= PRIVATE_DNS_SDK,
            Change::Compile => info.sdk >= COMPILE_SDK,
            _ => true,
        }
    }
}

/// What the person chose for this run. A scale of 0.0 means "leave as is".
#[derive(Default)]
pub struct Plan {
    pub font: f64,
    pub display: f64,
    pub background: Vec<String>,
    pub mute: Vec<String>,
    pub changes: Vec<Change>,
}

impl Plan {
    pub fn wants(&self, change: Change) -> bool {
        self.changes.contains(&change)
    }
}

/// Smoothness, battery, text size, app limits and ad personalization, as far as the plan asks for them.
pub fn apply(tw: &mut Tweaks, info: &DeviceInfo, plan: &Plan, paths: &Paths) {
    ui::step("Smoothness and battery settings");
    settings::apply_speed_settings(tw, plan);
    settings::apply_text_size(tw, plan);
    apps::apply_background(tw, info, plan, paths);
    apps::apply_mute(tw, info, plan);
    if plan.wants(Change::AdPersonalization) && screen_flows::set_ad_personalization_off(tw.adb) {
        ui::ok("Google ad personalization off (ads no longer follow your interests)");
    }
}
