//! Talking to the phone: adb itself, what the phone is, its packages, its screen, the USB bus, and root
//! when the owner has it. Raw command output is parsed here, once; the other layers trust these types.

pub mod adb;
pub mod brands;
pub mod info;
pub mod packages;
pub mod root;
pub mod screen;
#[cfg(test)]
mod tests;
pub mod usb;

use adb::Adb;
use info::DeviceInfo;
use root::Root;

/// Everything the actions need about the connected phone in one value.
pub struct Phone {
    pub adb: Adb,
    pub info: DeviceInfo,
    pub root: Option<Root>,
}
