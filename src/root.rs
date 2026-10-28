//! Optional extras for phones whose owner rooted them (Magisk, KernelSU). Read-only.
//!
//! We never try to gain root and ship no exploit. Detection is silent: on a phone without root nothing
//! here runs, asks or prints - a `Root` value exists only when root really works.

use crate::adb::Adb;
use crate::config;

/// Proof that this phone has working root. Only `probe` can make one.
pub struct Root(());

pub enum Probe {
    /// No `su` on the phone - the normal case. Nothing root-related is shown.
    Absent,
    /// `su` exists but the shell was not granted root (Magisk/KernelSU asked and was refused or ignored).
    Denied,
    Ready(Root),
}

impl Root {
    pub fn probe(adb: &Adb) -> Probe {
        // `command -v` first: on a phone without root this never touches `su` at all.
        if adb.sh("command -v su 2>/dev/null").trim().is_empty() {
            return Probe::Absent;
        }
        let id = adb.sh(&format!("timeout {} su -c id 2>/dev/null", config::ROOT_PROBE_SECS));
        if id.contains("uid=0") {
            Probe::Ready(Root(()))
        } else {
            Probe::Denied
        }
    }

    /// Read a root-only file as text.
    pub fn read(&self, adb: &Adb, path: &str) -> String {
        adb.sh(&format!("su -c cat {path} 2>/dev/null")).trim().to_string()
    }

    /// Read the leading number of a root-only file.
    pub fn read_num(&self, adb: &Adb, path: &str) -> Option<i64> {
        leading_number(&self.read(adb, path))
    }
}

pub fn leading_number(text: &str) -> Option<i64> {
    let digits: String = text.trim().chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// CPU clusters whose allowed maximum frequency is held below the hardware maximum - a deliberate cap.
pub fn cpu_caps(root: &Root, adb: &Adb) -> Vec<String> {
    let base = "/sys/devices/system/cpu/cpufreq";
    let mut caps = Vec::new();
    for policy in adb.sh(&format!("ls {base} 2>/dev/null")).lines().map(str::trim).filter(|p| p.starts_with("policy")) {
        let hw = root.read_num(adb, &format!("{base}/{policy}/cpuinfo_max_freq"));
        let cap = root.read_num(adb, &format!("{base}/{policy}/scaling_max_freq"));
        if let (Some(hw), Some(cap)) = (hw, cap) {
            if is_capped(hw, cap) {
                caps.push(format!(
                    "CPU {policy} is capped at {} MHz of {} MHz hardware max - a deliberate frequency limit.",
                    cap / 1000,
                    hw / 1000
                ));
            }
        }
    }
    caps
}

/// Below 95% of the hardware maximum counts as a cap (small differences are normal rounding).
pub fn is_capped(hardware_max: i64, allowed_max: i64) -> bool {
    allowed_max * 100 < hardware_max * 95
}
