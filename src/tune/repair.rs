//! Put back to Android's defaults the settings other "optimizers" left behind (see `inspect::leftovers`).
//! Each original value is recorded like any other change, so Restore can bring it back.

use super::settings::Tweaks;
use crate::inspect::leftovers;
use crate::ui;

pub fn apply(tw: &mut Tweaks) {
    for l in leftovers::on_phone(tw.adb) {
        if tw.set(l.ns, l.key, l.healthy) {
            ui::ok(&format!("Repaired: {}", l.what));
        }
    }
}
