//! Locating and driving the `adb` executable. Nothing else in the project spawns adb directly.

use crate::config;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Adb {
    path: PathBuf,
    serial: Option<String>,
}

/// One entry from `adb devices`.
pub struct DeviceEntry {
    pub serial: String,
    pub state: String,
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "adb.exe"
    } else {
        "adb"
    }
}

/// Run a command and return stdout+stderr merged, CRLF-normalised and right-trimmed (mirrors `2>&1`).
fn capture(cmd: &mut Command) -> String {
    match cmd.output() {
        Ok(o) => {
            let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&o.stderr));
            s.replace('\r', "").trim_end().to_string()
        }
        Err(_) => String::new(),
    }
}

impl Adb {
    /// Find adb: an override, then PATH, SDK locations, the system drive and the project folder.
    pub fn locate(override_path: Option<&str>, project_root: Option<&Path>) -> Option<Adb> {
        if let Some(p) = override_path {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return Some(Adb::new(pb));
            }
        }
        // On PATH.
        if Command::new(exe_name()).arg("version").output().map(|o| o.status.success()).unwrap_or(false) {
            return Some(Adb::new(PathBuf::from(exe_name())));
        }

        let mut candidates: Vec<PathBuf> = Vec::new();
        for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
            if let Ok(dir) = std::env::var(var) {
                candidates.push(Path::new(&dir).join("platform-tools").join(exe_name()));
            }
        }
        if cfg!(windows) {
            if let Ok(la) = std::env::var("LOCALAPPDATA") {
                candidates.push(Path::new(&la).join("Android/Sdk/platform-tools").join(exe_name()));
            }
            // Respect the real system drive instead of assuming C:.
            let sys = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
            candidates.push(Path::new(&format!("{sys}\\")).join("platform-tools").join(exe_name()));
            candidates.push(Path::new(&format!("{sys}\\")).join("adb").join(exe_name()));
        } else if let Ok(home) = std::env::var("HOME") {
            candidates.push(Path::new(&home).join("Library/Android/sdk/platform-tools").join(exe_name()));
            candidates.push(Path::new(&home).join("Android/Sdk/platform-tools").join(exe_name()));
            candidates.push(PathBuf::from("/usr/bin/adb"));
            candidates.push(PathBuf::from("/usr/local/bin/adb"));
            candidates.push(PathBuf::from("/opt/homebrew/bin/adb"));
        }
        if let Some(root) = project_root {
            candidates.push(root.join("platform-tools").join(exe_name()));
        }
        candidates.into_iter().find(|c| c.is_file()).map(Adb::new)
    }

    fn new(path: PathBuf) -> Adb {
        Adb { path, serial: None }
    }

    pub fn set_serial(&mut self, serial: impl Into<String>) {
        self.serial = Some(serial.into());
    }

    pub fn serial(&self) -> Option<&str> {
        self.serial.as_deref()
    }

    /// Run `adb <args...>` (no implicit device selection).
    pub fn run(&self, args: &[&str]) -> String {
        capture(Command::new(&self.path).args(args))
    }

    /// Run a command in the device shell (`adb -s <serial> shell <cmd>`).
    /// The command is passed as one argument, so the device's own shell handles pipes and redirects.
    pub fn sh(&self, cmd: &str) -> String {
        let mut c = Command::new(&self.path);
        if let Some(s) = &self.serial {
            c.args(["-s", s]);
        }
        c.args(["shell", cmd]);
        capture(&mut c)
    }

    pub fn start_server(&self) {
        let _ = self.run(&["start-server"]);
    }

    /// Run a device-shell command and call `on_line` for each line of output as it arrives (for progress).
    pub fn shell_stream(&self, cmd: &str, mut on_line: impl FnMut(&str)) {
        let mut c = Command::new(&self.path);
        if let Some(s) = &self.serial {
            c.args(["-s", s]);
        }
        c.args(["shell", cmd]).stdout(Stdio::piped()).stderr(Stdio::null());
        if let Ok(mut child) = c.spawn() {
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    on_line(line.trim_end_matches('\r'));
                }
            }
            let _ = child.wait();
        }
    }

    /// Read a settings value (`settings get <ns> <key>`), trimmed.
    pub fn get_setting(&self, ns: &str, key: &str) -> String {
        self.sh(&format!("settings get {ns} {key}")).trim().to_string()
    }

    pub fn reboot(&self) {
        if let Some(s) = &self.serial {
            let _ = self.run(&["-s", s, "reboot"]);
        } else {
            let _ = self.run(&["reboot"]);
        }
    }

    /// Read a file off the device as text (`adb exec-out cat <remote>`).
    pub fn pull_text(&self, remote: &str) -> String {
        let mut c = Command::new(&self.path);
        if let Some(s) = &self.serial {
            c.args(["-s", s]);
        }
        c.args(["exec-out", "cat", remote]);
        capture(&mut c)
    }

    /// Push a local file to the device.
    pub fn push(&self, local: &Path, remote: &str) {
        if let Some(s) = &self.serial {
            let _ = self.run(&["-s", s, "push", &local.to_string_lossy(), remote]);
        }
    }

    pub fn devices(&self) -> Vec<DeviceEntry> {
        let mut out = Vec::new();
        for line in self.run(&["devices"]).lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("List of devices") || line.starts_with('*') {
                continue;
            }
            if let Some((serial, rest)) = line.split_once(char::is_whitespace) {
                let state = rest.split_whitespace().next().unwrap_or("").to_string();
                if !state.is_empty() {
                    out.push(DeviceEntry { serial: serial.to_string(), state });
                }
            }
        }
        out
    }

    /// Wait up to 15 minutes for a single ready device. `wanted` filters by serial.
    /// `on_absent` runs on every poll while no phone is visible, so the caller can refresh its guidance.
    pub fn wait_for_device(&self, wanted: Option<&str>, mut on_absent: impl FnMut()) -> Option<String> {
        let deadline = Instant::now() + Duration::from_secs(config::DEVICE_WAIT_SECS);
        let mut last_state = String::new();
        while Instant::now() < deadline {
            let mut devs = self.devices();
            if let Some(w) = wanted {
                devs.retain(|d| d.serial == w);
            }
            let ready: Vec<&DeviceEntry> = devs.iter().filter(|d| d.state == "device").collect();
            if ready.len() == 1 {
                return Some(ready[0].serial.clone());
            }
            if ready.len() > 1 {
                crate::ui::warn("Several phones are connected - pass --serial <id>:");
                for d in &ready {
                    crate::ui::dim(&d.serial);
                }
                return None;
            }
            let state = devs.first().map(|d| d.state.clone()).unwrap_or_else(|| "none".into());
            if state == "none" {
                on_absent();
            }
            if state != last_state {
                match state.as_str() {
                    "unauthorized" => crate::ui::warn(
                        "The phone shows \"Allow USB debugging?\" - tick \"Always allow\" and tap Allow",
                    ),
                    "offline" => crate::ui::warn("Phone is \"offline\" - unplug and plug the cable back in"),
                    "recovery" => crate::ui::warn("Phone is in recovery. Choose Reboot and wait for Android to start"),
                    "none" => crate::ui::dim("Waiting for the phone... (Ctrl+C to quit)"),
                    other => crate::ui::warn(&format!("Phone state: {other}")),
                }
                last_state = state;
            }
            std::thread::sleep(Duration::from_secs(config::DEVICE_POLL_SECS));
        }
        None
    }

    /// Wireless debugging. Pair (optional) then connect. Returns the connected address on success.
    pub fn connect_wifi(&self, pair: Option<&str>, code: Option<&str>, connect: Option<&str>) -> Option<String> {
        if let Some(addr) = pair {
            let r = self.run(&["pair", addr, code.unwrap_or("")]);
            if r.contains("Successfully paired") {
                crate::ui::ok("Paired");
            } else {
                crate::ui::fail(&format!("Pairing failed: {r}"));
                return None;
            }
        }
        let mut target = connect.map(|s| s.to_string());
        if target.is_none() {
            // Paired phones announce themselves over mDNS.
            let ip = pair.and_then(|p| p.split(':').next()).unwrap_or("");
            for line in self.run(&["mdns", "services"]).lines() {
                if line.contains("_adb-tls-connect._tcp") {
                    if let Some(addr) = line.split_whitespace().find(|t| t.contains(':') && t.contains('.')) {
                        if ip.is_empty() || addr.starts_with(ip) {
                            target = Some(addr.to_string());
                            break;
                        }
                    }
                }
            }
        }
        let addr = target?;
        let r = self.run(&["connect", &addr]);
        if r.contains("connected to") {
            crate::ui::ok(&format!("Connected over Wi-Fi: {addr}"));
            Some(addr)
        } else {
            crate::ui::fail(&format!("Could not connect: {r}"));
            None
        }
    }
}
