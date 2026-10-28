//! Storage / RAM / battery / heat / stuck-key diagnostics and the warnings drawn from them.

use crate::adb::Adb;
use crate::device::{DeviceInfo, Packages, Phone};
use crate::root::leading_number;
use crate::ui;
use crate::util;

/// Volume presses in a row (from the system itself) that mean a key is physically stuck.
const STUCK_KEY_PRESSES: usize = 30;
/// ...all within this window...
const STUCK_KEY_SPAN_MS: i64 = 5_000;
/// ...and the last one this recently.
const STUCK_KEY_MAX_AGE_MS: i64 = 60_000;

const BATTERY_SYSFS: &str = "/sys/class/power_supply/battery";

#[derive(Default)]
pub struct Storage {
    pub total_mb: i64,
    pub free_mb: i64,
    pub pct: i64,
}

#[derive(Default)]
pub struct Diag {
    pub ram_total_mb: i64,
    pub ram_free_mb: i64,
    pub storage: Option<Storage>,
    pub battery_level: Option<i64>,
    pub battery_temp_c: f64,
    pub battery_health: i64,
    pub charging: bool,
    pub battery_capacity_pct: i64,
    pub battery_mah: String,
    pub battery_cycles: i64,
    pub thermal_status: i64,
    pub stuck_volume_key: Option<&'static str>,
    pub packages: usize,
    pub user_apps: usize,
}

/// The first integer after `label` on any line.
pub fn num_after(text: &str, label: &str) -> Option<i64> {
    text.lines().find_map(|line| {
        let tail = &line[line.find(label)? + label.len()..];
        leading_number(tail)
    })
}

/// `df -k /data` -> sizes in MB and percent used.
pub fn parse_df(text: &str) -> Option<Storage> {
    text.lines().find_map(|line| {
        let tok: Vec<&str> = line.split_whitespace().collect();
        let p = tok
            .iter()
            .position(|t| t.len() > 1 && t.ends_with('%') && t[..t.len() - 1].chars().all(|c| c.is_ascii_digit()))?;
        if p < 3 {
            return None;
        }
        let total: i64 = tok[p - 3].parse().ok()?;
        let avail: i64 = tok[p - 1].parse().ok()?;
        let pct: i64 = tok[p].trim_end_matches('%').parse().ok()?;
        Some(Storage { total_mb: total / 1024, free_mb: avail / 1024, pct })
    })
}

pub fn storage(adb: &Adb) -> Option<Storage> {
    parse_df(&adb.sh("df -k /data"))
}

/// "HH:MM:SS" -> milliseconds since midnight.
pub fn parse_hms(s: &str) -> Option<i64> {
    let p: Vec<i64> = s.trim().split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    (p.len() == 3).then(|| (p[0] * 3600 + p[1] * 60 + p[2]) * 1000)
}

/// A "HH:MM:SS:mmm" timestamp anywhere in the line -> milliseconds since midnight.
pub fn parse_hms_ms(line: &str) -> Option<i64> {
    line.split_whitespace().find_map(|tok| {
        let d: Vec<&str> = tok.split(':').collect();
        let shape = d.len() == 4 && d[..3].iter().all(|p| p.len() == 2) && d[3].len() == 3;
        if !shape {
            return None;
        }
        let n: Vec<i64> = d.iter().map(|p| p.parse().ok()).collect::<Option<_>>()?;
        Some(n[0] * 3_600_000 + n[1] * 60_000 + n[2] * 1000 + n[3])
    })
}

/// A stuck volume key: the system "presses" it every ~50 ms. Held during boot, it sends many phones into
/// recovery - it looks like a boot loop, and a factory reset does not fix it.
pub fn stuck_key(audio_dump: &str, now_ms: i64) -> Option<&'static str> {
    let presses: Vec<&str> = audio_dump
        .lines()
        .filter(|l| l.contains("adjustSuggestedStreamVolume(") && l.contains("from android/android"))
        .filter(|l| l.contains("dir:ADJUST_RAISE") || l.contains("dir:ADJUST_LOWER"))
        .collect();
    let tail = presses.get(presses.len().checked_sub(STUCK_KEY_PRESSES)?..)?;
    let times: Vec<i64> = tail.iter().map(|l| parse_hms_ms(l)).collect::<Option<_>>()?;
    let (first, last) = (times[0], *times.last()?);
    let recent = (0..=STUCK_KEY_MAX_AGE_MS).contains(&(now_ms - last));
    if last - first > STUCK_KEY_SPAN_MS || !recent {
        return None;
    }
    Some(if tail.last()?.contains("ADJUST_RAISE") { "Volume Up" } else { "Volume Down" })
}

/// Battery wear from `dumpsys batterystats`: (percent of design capacity, "learned of design mAh").
/// A learned value exactly equal to the design one means the phone never measured wear (Samsung) - unknown.
pub fn capacity_from_stats(stats: &str) -> Option<(i64, String)> {
    let design = num_after(stats, "Estimated battery capacity:").filter(|v| *v > 0)?;
    let learned = num_after(stats, "Max learned battery capacity:").filter(|v| *v > 0 && *v != design)?;
    Some(((100 * learned / design).min(100), format!("{learned} of {design} mAh")))
}

/// A number from sysfs; on a rooted phone, files closed to the shell are read through root.
fn read_sys(phone: &Phone, path: &str) -> Option<i64> {
    leading_number(&phone.adb.sh(&format!("cat {path} 2>/dev/null")))
        .or_else(|| phone.root.as_ref().and_then(|r| r.read_num(&phone.adb, path)))
}

fn fill_memory(d: &mut Diag, meminfo: &str) {
    d.ram_total_mb = num_after(meminfo, "MemTotal:").unwrap_or(0) / 1024;
    d.ram_free_mb = num_after(meminfo, "MemAvailable:").unwrap_or(0) / 1024;
}

fn fill_battery(d: &mut Diag, dump: &str) {
    d.battery_level = num_after(dump, "level:");
    d.battery_temp_c = num_after(dump, "temperature:").unwrap_or(0) as f64 / 10.0;
    d.battery_health = num_after(dump, "health:").unwrap_or(0);
    d.charging = dump.lines().map(str::trim).any(|l| {
        ["AC powered:", "USB powered:", "Wireless powered:"].iter().any(|k| l.starts_with(k)) && l.ends_with("true")
    });
}

fn fill_battery_wear(d: &mut Diag, phone: &Phone) {
    let full = read_sys(phone, &format!("{BATTERY_SYSFS}/charge_full"));
    let design = read_sys(phone, &format!("{BATTERY_SYSFS}/charge_full_design")).filter(|v| *v > 0);
    if let (Some(full), Some(design)) = (full, design) {
        d.battery_capacity_pct = (100 * full / design).min(100);
    } else if let Some((pct, mah)) = capacity_from_stats(
        &phone.adb.sh("dumpsys batterystats | grep -m3 -e Estimated.battery.capacity -e Max.learned.battery.capacity"),
    ) {
        // sysfs is often closed without root (Xiaomi) - Android's own battery stats are the fallback.
        d.battery_capacity_pct = pct;
        d.battery_mah = mah;
    }
    d.battery_cycles = read_sys(phone, &format!("{BATTERY_SYSFS}/cycle_count")).unwrap_or(0);
}

pub fn collect(phone: &Phone, pkgs: &Packages) -> Diag {
    let adb = &phone.adb;
    let mut d = Diag::default();
    fill_memory(&mut d, &adb.sh("cat /proc/meminfo"));
    d.storage = storage(adb);
    fill_battery(&mut d, &adb.sh("dumpsys battery"));
    fill_battery_wear(&mut d, phone);
    d.thermal_status = num_after(&adb.sh("dumpsys thermalservice"), "Thermal Status:").unwrap_or(0);
    d.stuck_volume_key = parse_hms(&adb.sh("date +%H:%M:%S")).and_then(|now| stuck_key(&adb.sh("dumpsys audio"), now));
    d.packages = pkgs.len();
    d.user_apps = pkgs.user_app_count();
    d
}

pub fn show(d: &Diag) {
    if d.ram_total_mb > 0 {
        ui::say(&format!("RAM:      {} free of {}", ui::format_mb(d.ram_free_mb), ui::format_mb(d.ram_total_mb)));
    }
    if let Some(s) = &d.storage {
        ui::say(&format!(
            "Storage:  {} free of {} ({}% used)",
            ui::format_mb(s.free_mb),
            ui::format_mb(s.total_mb),
            s.pct
        ));
    }
    if let Some(level) = d.battery_level {
        ui::say(&format!("Battery:  {level}%, {} C{}", d.battery_temp_c, battery_extra(d)));
    }
    ui::say(&format!("Apps:     {} in total, {} not part of the system", d.packages, d.user_apps));
}

fn battery_extra(d: &Diag) -> String {
    let mut extra = String::new();
    if d.battery_capacity_pct > 0 {
        extra.push_str(&format!(", health: ~{}% of original capacity", d.battery_capacity_pct));
    }
    if !d.battery_mah.is_empty() {
        extra.push_str(&format!(" ({})", d.battery_mah));
    }
    if d.battery_cycles > 0 {
        extra.push_str(&format!(", cycles: {}", d.battery_cycles));
    }
    extra
}

pub fn warnings(adb: &Adb, d: &Diag, info: &DeviceInfo) {
    if d.storage.as_ref().is_some_and(|s| s.pct >= 85) {
        ui::warn("Storage is almost full - a main cause of lag and freezes.");
        ui::dim("Move photos and videos to the cloud or a computer: keep at least 15-20% free.");
    }
    if d.battery_health != 0 && d.battery_health != 2 {
        ui::warn("The system reports a battery problem: overheating, sudden shutdowns and slowdowns.");
        ui::dim("A battery replacement usually costs a fraction of a new phone.");
    }
    if (1..80).contains(&d.battery_capacity_pct) {
        ui::warn(&format!(
            "The battery is worn (~{}% capacity left) - a common cause of slowdowns and heat.",
            d.battery_capacity_pct
        ));
    }
    if let Some(key) = d.stuck_volume_key {
        ui::warn(&format!("The {key} key seems stuck: the phone \"presses\" it many times a second."));
        ui::dim(
            "Remove the case, press the key a few times, clean the gap; otherwise a repair shop replaces it cheaply.",
        );
        ui::dim("Do not reboot while it is stuck: with the key held the phone boots into recovery.");
    }
    if d.battery_temp_c >= 42.0 {
        ui::warn(&format!(
            "The phone is hot ({} C) - remove the case, do not charge and play at once.",
            d.battery_temp_c
        ));
    }
    if d.thermal_status >= 2 {
        ui::warn("The system is already slowing down because of heat.");
    }
    if d.ram_total_mb != 0 && d.ram_total_mb < 3200 {
        ui::warn("Little RAM (3 GB or less): keep fewer apps, prefer Lite/Go versions.");
    }
    let patch = util::parse_ymd_to_days(&info.patch);
    let today = util::parse_ymd_to_days(adb.sh("date +%Y-%m-%d").trim());
    if let (Some(patch), Some(today)) = (patch, today) {
        if today - patch > 730 {
            ui::warn(&format!("No security updates for more than two years (patch {}).", info.patch));
        }
    }
}
