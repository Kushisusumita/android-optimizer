//! What the phone is built from (SoC, CPU cores and clocks, GPU) and how loaded it is right now,
//! plus one honest speed measurement: how long a standard app takes to open from cold.

use crate::adb::Adb;
use crate::device::Phone;
use crate::diagnostics::{num_after, Diag};
use crate::root::leading_number;
use crate::ui;
use std::collections::BTreeMap;
use std::thread::sleep;
use std::time::Duration;

/// Cold starts measured; the median is reported so one hiccup does not skew the result.
const LAUNCH_RUNS: usize = 3;
const CPU_SAMPLE_MS: u64 = 500;
const TOP_PROCESSES: usize = 5;

/// GPU busy files that some vendors leave readable (Adreno first, then generic).
const GPU_BUSY_FILES: &[&str] = &[
    "/sys/class/kgsl/kgsl-3d0/gpu_busy_percentage",
    "/sys/kernel/gpu/gpu_busy",
    "/sys/devices/platform/gpusysfs/gpu_busy",
];

/// "GLES: Qualcomm, Adreno (TM) 610, OpenGL ES 3.2 ..." -> "Qualcomm Adreno (TM) 610".
pub fn parse_gpu(surfaceflinger: &str) -> Option<String> {
    let line = surfaceflinger.lines().find_map(|l| l.trim().strip_prefix("GLES:"))?;
    let mut parts = line.split(',').map(str::trim);
    let vendor = parts.next()?;
    let renderer = parts.next()?;
    Some(format!("{vendor} {renderer}").trim().to_string())
}

/// Per-core max frequencies (kHz, one per line) -> "6 x 1.8 GHz + 2 x 2.2 GHz".
pub fn describe_clusters(freqs_khz: &str) -> Option<String> {
    let mut clusters: BTreeMap<i64, usize> = BTreeMap::new();
    for f in freqs_khz.lines().filter_map(leading_number).filter(|f| *f > 0) {
        *clusters.entry(f).or_default() += 1;
    }
    if clusters.is_empty() {
        return None;
    }
    let parts: Vec<String> =
        clusters.iter().map(|(khz, n)| format!("{n} x {:.1} GHz", *khz as f64 / 1_000_000.0)).collect();
    Some(parts.join(" + "))
}

pub fn show_specs(phone: &Phone, d: &Diag) {
    let adb = &phone.adb;
    ui::step("Hardware");
    if !phone.info.soc.is_empty() {
        ui::say(&format!("Chip:     {}", phone.info.soc));
    }
    if let Some(cpu) =
        describe_clusters(&adb.sh("cat /sys/devices/system/cpu/cpu*/cpufreq/cpuinfo_max_freq 2>/dev/null"))
    {
        ui::say(&format!("CPU:      {cpu}"));
    }
    if let Some(gpu) = parse_gpu(&adb.sh("dumpsys SurfaceFlinger | grep -m1 GLES:")) {
        ui::say(&format!("GPU:      {gpu}"));
    }
    if d.ram_total_mb > 0 {
        ui::say(&format!("RAM:      {}", ui::format_mb(d.ram_total_mb)));
    }
}

/// (idle, total) jiffies from the first "cpu " line of /proc/stat.
pub fn cpu_times(stat: &str) -> Option<(i64, i64)> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let n: Vec<i64> = line.split_whitespace().skip(1).filter_map(|t| t.parse().ok()).collect();
    let idle = n.get(3)? + n.get(4).copied().unwrap_or(0); // idle + iowait
    Some((idle, n.iter().sum()))
}

/// Busy percent between two /proc/stat samples.
pub fn busy_percent(before: (i64, i64), after: (i64, i64)) -> Option<i64> {
    let total = after.1 - before.1;
    let idle = after.0 - before.0;
    (total > 0).then(|| (100 * (total - idle) / total).clamp(0, 100))
}

fn cpu_busy(adb: &Adb) -> Option<i64> {
    let first = cpu_times(&adb.sh("cat /proc/stat"))?;
    sleep(Duration::from_millis(CPU_SAMPLE_MS));
    busy_percent(first, cpu_times(&adb.sh("cat /proc/stat"))?)
}

/// GPU load when the vendor exposes it; on a rooted phone the same files are read through root.
fn gpu_busy(phone: &Phone) -> Option<i64> {
    GPU_BUSY_FILES.iter().find_map(|path| {
        leading_number(&phone.adb.sh(&format!("cat {path} 2>/dev/null")))
            .or_else(|| phone.root.as_ref().and_then(|r| r.read_num(&phone.adb, path)))
            .filter(|v| *v <= 100)
    })
}

pub fn show_load(phone: &Phone, d: &Diag) {
    ui::step("Current load");
    if let Some(c) = cpu_busy(&phone.adb) {
        ui::say(&format!("CPU busy: {c}%"));
    }
    if let Some(g) = gpu_busy(phone) {
        ui::say(&format!("GPU busy: {g}%"));
    }
    if d.ram_total_mb > 0 {
        let used = d.ram_total_mb - d.ram_free_mb;
        ui::say(&format!("RAM used: {} of {}", ui::format_mb(used), ui::format_mb(d.ram_total_mb)));
    }
    // Two samples a second apart: the first one has no CPU time to compare with yet. One extra row for top itself.
    let top = phone.adb.sh(&format!("top -b -n 2 -d 1 -m {} -o %CPU,NAME -s 1", TOP_PROCESSES + 1));
    match busiest(&top) {
        Some(rows) if rows.is_empty() => ui::dim("Nothing is using the CPU right now."),
        Some(rows) => {
            ui::dim("Busiest processes (CPU %, name):");
            for (cpu, name) in rows.iter().take(TOP_PROCESSES) {
                ui::dim(&format!("  {cpu:>5.1}  {name}"));
            }
        }
        None => {}
    }
}

/// Processes using CPU in the last sample of `top` output, without top itself. None if there is no table.
pub fn busiest(top: &str) -> Option<Vec<(f64, String)>> {
    let lines: Vec<&str> = top.lines().collect();
    let header = lines.iter().rposition(|l| l.contains("%CPU"))?;
    let rows = lines[header + 1..]
        .iter()
        .filter_map(|l| {
            let (cpu, name) = l.trim().split_once(char::is_whitespace)?;
            Some((cpu.parse::<f64>().ok()?, name.trim().to_string()))
        })
        .filter(|(cpu, name)| *cpu > 0.0 && name != "top")
        .collect();
    Some(rows)
}

/// Cold-start time of the Settings app in ms (median of a few runs). Opens Settings briefly.
pub fn launch_ms(adb: &Adb) -> Option<i64> {
    let component = adb.sh("cmd package resolve-activity --brief -a android.settings.SETTINGS");
    let package = component.lines().rev().find_map(|l| l.trim().split_once('/').map(|(p, _)| p.to_string()))?;
    let mut runs: Vec<i64> = (0..LAUNCH_RUNS)
        .filter_map(|_| {
            let _ = adb.sh(&format!("am force-stop {package}"));
            sleep(Duration::from_millis(800));
            let out = adb.sh("am start -W -a android.settings.SETTINGS");
            num_after(&out, "TotalTime:").or_else(|| num_after(&out, "WaitTime:"))
        })
        .collect();
    let _ = adb.sh("input keyevent KEYCODE_HOME");
    let _ = adb.sh(&format!("am force-stop {package}"));
    runs.sort_unstable();
    runs.get(runs.len() / 2).copied()
}
