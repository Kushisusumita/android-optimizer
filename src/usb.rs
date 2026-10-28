//! Recognising a phone on the USB bus before USB debugging is on, from the computer's own device list,
//! so the connection steps can be brand-exact from the very first screen.

use crate::brands::Brand;
use std::process::Command;

pub struct UsbPhone {
    pub brand: Brand,
    pub name: String,
}

/// Single quotes only: Windows PowerShell 5.1 mangles double quotes passed on its command line.
const WINDOWS_QUERY: &str =
    "Get-CimInstance Win32_PnPEntity | Where-Object { $_.DeviceID -like 'USB*VID_*' } | ForEach-Object { $_.DeviceID + '|' + $_.Name }";

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn hex4(s: &str) -> Option<u16> {
    u16::from_str_radix(s.get(..4)?, 16).ok()
}

/// `DEVICEID|Name` lines from Win32_PnPEntity.
pub fn parse_windows(text: &str) -> Vec<(u16, String)> {
    text.lines()
        .filter_map(|l| {
            let (id, name) = l.trim().split_once('|')?;
            let vid = hex4(&id[id.find("VID_")? + 4..])?;
            Some((vid, name.trim().to_string()))
        })
        .collect()
}

/// `lsusb`: "Bus 001 Device 005: ID 2717:ff48 Xiaomi Inc. Mi/Redmi series (MTP)".
pub fn parse_lsusb(text: &str) -> Vec<(u16, String)> {
    text.lines()
        .filter_map(|l| {
            let rest = &l[l.find(" ID ")? + 4..];
            let vid = hex4(rest)?;
            let name = rest.split_once(' ').map(|(_, n)| n.trim()).unwrap_or("");
            Some((vid, name.to_string()))
        })
        .collect()
}

/// `system_profiler SPUSBDataType`: a "Name:" header line, then "Vendor ID: 0x2717 (...)".
pub fn parse_macos(text: &str) -> Vec<(u16, String)> {
    let mut found = Vec::new();
    let mut name = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(header) = line.strip_suffix(':') {
            name = header.to_string();
        } else if let Some(v) = line.strip_prefix("Vendor ID: 0x") {
            if let Some(vid) = hex4(v) {
                found.push((vid, name.clone()));
            }
        }
    }
    found
}

fn usb_devices() -> Vec<(u16, String)> {
    if cfg!(windows) {
        parse_windows(&output("powershell", &["-NoProfile", "-NonInteractive", "-Command", WINDOWS_QUERY]))
    } else if cfg!(target_os = "macos") {
        parse_macos(&output("system_profiler", &["SPUSBDataType"]))
    } else {
        parse_lsusb(&output("lsusb", &[]))
    }
}

/// Pick the phone among USB entries: a known maker, preferring a real model name over "USB ... Device".
pub fn pick_phone(devices: Vec<(u16, String)>) -> Option<UsbPhone> {
    let mut phones: Vec<UsbPhone> = devices
        .into_iter()
        .filter_map(|(vid, name)| Brand::from_usb_vendor(vid).map(|brand| UsbPhone { brand, name }))
        .collect();
    phones.sort_by_key(|p| is_driver_interface(&p.name));
    phones.into_iter().next()
}

/// Words in the names Windows gives a phone's driver interfaces ("SAMSUNG Android ADB Interface",
/// "... USB Modem"); the file-transfer entry carries the model name instead ("Galaxy A12").
const INTERFACE_WORDS: &[&str] = &["usb", "adb", "interface", "modem", "composite", "driver", "serial"];

fn is_driver_interface(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    INTERFACE_WORDS.iter().any(|w| lower.contains(w))
}

pub fn find_phone() -> Option<UsbPhone> {
    pick_phone(usb_devices())
}
