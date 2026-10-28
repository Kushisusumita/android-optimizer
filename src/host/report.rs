//! The per-run report folder under `reports/`: device metadata and the undo records used by Restore.

use super::paths::Paths;
use crate::phone::info::DeviceInfo;
use crate::phone::packages::Packages;
use crate::util;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Report {
    dir: PathBuf,
}

impl Report {
    pub fn create(paths: &Paths, info: &DeviceInfo, tag: &str) -> Report {
        let name = format!(
            "{}_{}_{}",
            util::now_utc_compact(),
            util::slug(&format!("{}_{}", info.brand_name, info.model)),
            tag
        );
        let dir = paths.reports.join(name);
        let _ = fs::create_dir_all(&dir);
        let meta = format!(
            "device_id={}\nfingerprint={}\nbrand={}\nmodel={}\nname={}\nandroid={}\nsdk={}\n",
            info.device_id, info.fingerprint, info.brand_name, info.model, info.name, info.android, info.sdk
        );
        let _ = fs::write(dir.join("device.txt"), meta);
        Report { dir }
    }

    pub fn write(&self, file: &str, body: &str) {
        let _ = fs::write(self.dir.join(file), body);
    }

    /// One package name per line (already sorted: the set is ordered).
    pub fn write_packages(&self, file: &str, pkgs: &Packages) {
        self.write(file, &pkgs.names().map(|n| format!("{n}\n")).collect::<String>());
    }

    pub fn append(&self, file: &str, line: &str) {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(self.dir.join(file)) {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }
}

/// All report folders of this device, oldest first.
pub fn device_reports(paths: &Paths, device_id: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = match fs::read_dir(&paths.reports) {
        Ok(d) => d.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect(),
        Err(_) => return Vec::new(),
    };
    dirs.retain(|d| {
        fs::read_to_string(d.join("device.txt"))
            .map(|t| t.lines().any(|l| l == format!("device_id={device_id}")))
            .unwrap_or(false)
    });
    dirs.sort();
    dirs
}

/// The firmware fingerprint recorded in this device's most recent report (to detect a system update).
pub fn last_fingerprint(paths: &Paths, device_id: &str) -> Option<String> {
    device_reports(paths, device_id).last().and_then(|d| {
        fs::read_to_string(d.join("device.txt"))
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("fingerprint=").map(|s| s.to_string())))
    })
}

/// First recorded value per key across all of this device's reports (= the original value).
/// Each line is `key<TAB>value` after an optional extra key column handled by the caller.
pub fn read_first_values(dirs: &[PathBuf], file: &str, key_cols: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for d in dirs {
        let text = fs::read_to_string(d.join(file)).unwrap_or_default();
        for line in text.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < key_cols {
                continue;
            }
            let key = cols[..key_cols].join("\t");
            if seen.insert(key.clone()) {
                let val = cols.get(key_cols).copied().unwrap_or("").to_string();
                out.push((key, val));
            }
        }
    }
    out
}

/// Apps the person installed since this phone's previous run (from that run's package list).
pub fn new_since_last_run(paths: &Paths, device_id: &str, pkgs: &Packages) -> Vec<String> {
    let Some(last) =
        device_reports(paths, device_id).into_iter().rev().find(|d| d.join("packages-after.txt").is_file())
    else {
        return Vec::new();
    };
    let old: HashSet<String> = std::fs::read_to_string(last.join("packages-after.txt"))
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect();
    pkgs.values().filter(|p| !p.system && p.enabled && !old.contains(&p.name)).map(|p| p.name.clone()).collect()
}
