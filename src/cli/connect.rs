//! Getting to a connected phone: adb itself (downloaded if missing), brand-exact steps while waiting,
//! Wi-Fi pairing, and the silent root probe.

use crate::config::{self, Opts};
use crate::host::paths::Paths;
use crate::host::platform_tools;
use crate::host::report;
use crate::phone::adb::Adb;
use crate::phone::brands::Brand;
use crate::phone::info::DeviceInfo;
use crate::phone::root::{Probe, Root};
use crate::phone::usb::{self, UsbPhone};
use crate::phone::Phone;
use crate::ui;

const NO_ADB: &str =
    "adb was not found. Install Android platform-tools (or put it in ./platform-tools) or pass --adb <path>.";

/// adb from the computer, or Google's official one downloaded into the project folder with consent.
pub fn locate_adb(paths: &Paths, opts: &Opts) -> Result<Adb, String> {
    if let Some(adb) = Adb::locate(opts.adb_path.as_deref(), Some(&paths.root)) {
        return Ok(adb);
    }
    ui::warn("adb (Android platform-tools) is not installed on this computer.");
    if !ui::ask_yes_no("Download Google's official platform-tools (about 7 MB) into this folder?", true, opts.yes) {
        return Err(NO_ADB.into());
    }
    ui::say(&format!("Downloading {}", platform_tools::download_url()));
    if !platform_tools::install(&paths.root) {
        return Err(format!("The download failed. Check the internet connection. {NO_ADB}"));
    }
    ui::ok("platform-tools installed");
    Adb::locate(None, Some(&paths.root)).ok_or_else(|| NO_ADB.into())
}

/// Brand-exact connection steps that update as soon as a phone shows up on the USB bus.
#[derive(Default)]
struct ConnectGuide {
    polls: u32,
    shown: Option<Option<Brand>>,
}

impl ConnectGuide {
    fn refresh(&mut self) {
        let due = self.polls.is_multiple_of(config::USB_SCAN_EVERY_POLLS);
        self.polls += 1;
        if !due {
            return;
        }
        let found = usb::find_phone();
        let key = found.as_ref().map(|p| p.brand);
        if self.shown != Some(key) {
            self.shown = Some(key);
            show_connect_steps(found);
        }
    }
}

fn show_connect_steps(found: Option<UsbPhone>) {
    let brand = match &found {
        Some(p) => {
            ui::step(&format!("Found on USB: {} ({}). Now turn on USB debugging:", p.name, p.brand.title()));
            p.brand
        }
        None => {
            ui::step("Connect the phone with a USB data cable (some cables only charge)");
            ui::dim("The exact steps for your phone's brand appear as soon as it is plugged in.");
            Brand::Other
        }
    };
    for line in brand.connect_steps() {
        ui::say(&line);
    }
    ui::dim("No working cable? Android 11+: Developer options -> Wireless debugging -> Pair device with pairing code,");
    ui::dim("then start again with: --pair <ip:port> --code <code>");
}

pub fn connect(mut adb: Adb, opts: &Opts) -> Result<Phone, String> {
    let wireless = (opts.pair.is_some() || opts.connect.is_some())
        .then(|| adb.connect_wifi(opts.pair.as_deref(), opts.pair_code.as_deref(), opts.connect.as_deref()))
        .flatten();
    let wanted = wireless.or_else(|| opts.serial.clone());
    let mut guide = ConnectGuide::default();
    let serial = adb.wait_for_device(wanted.as_deref(), || guide.refresh()).ok_or("No phone connected.")?;
    adb.set_serial(serial);
    let info = DeviceInfo::load(&adb);
    let root = detect_root(&adb);
    Ok(Phone { adb, info, root })
}

/// Silent unless root is really there: phones without root see nothing about it.
fn detect_root(adb: &Adb) -> Option<Root> {
    match Root::probe(adb) {
        Probe::Ready(root) => {
            ui::ok("Root found - extra read-only checks are on (CPU limits, GPU load, battery wear)");
            Some(root)
        }
        Probe::Denied => {
            ui::dim(
                "Root is installed, but Shell has no access - allow \"Shell\" in Magisk/KernelSU for extra checks.",
            );
            None
        }
        Probe::Absent => None,
    }
}

pub fn introduce(paths: &Paths, info: &DeviceInfo) {
    ui::step(&format!("Phone: {}", info.title()));
    ui::say(&format!("{} - Android {} (API {}), {}", info.brand.title(), info.android, info.sdk, info.rom));
    let previous = report::last_fingerprint(paths, &info.device_id);
    if previous.is_some_and(|fp| !fp.is_empty() && fp != info.fingerprint) {
        ui::warn("The system was updated since the last run - updates often bring removed apps back.");
    }
}
