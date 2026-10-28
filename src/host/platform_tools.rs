//! Getting Google's official Android platform-tools (adb) when it is missing, with the person's consent.
//! Uses the download and unpack tools the operating system already has, so no extra dependency.

use std::path::{Path, PathBuf};
use std::process::Command;

const BASE_URL: &str = "https://dl.google.com/android/repository/platform-tools-latest-";

fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

pub fn download_url() -> String {
    format!("{BASE_URL}{}.zip", os_name())
}

/// On Windows, the curl and tar (which unpacks zip) built into Windows 10+, not whatever is first on PATH.
fn system_tool(name: &str) -> PathBuf {
    match std::env::var("SystemRoot") {
        Ok(root) if cfg!(windows) => Path::new(&root).join("System32").join(format!("{name}.exe")),
        _ => PathBuf::from(name),
    }
}

fn succeeds(program: &Path, args: &[&str]) -> bool {
    Command::new(program).args(args).status().map(|s| s.success()).unwrap_or(false)
}

/// Download and unpack into `<root>/platform-tools`. True when adb is there afterwards.
pub fn install(root: &Path) -> bool {
    let zip = std::env::temp_dir().join("android-optimizer-platform-tools.zip");
    let (zip_s, root_s) = (zip.to_string_lossy().into_owned(), root.to_string_lossy().into_owned());
    let downloaded = succeeds(&system_tool("curl"), &["-fsSL", "-o", &zip_s, &download_url()]);
    let unpacked = downloaded
        && (succeeds(&system_tool("tar"), &["-xf", &zip_s, "-C", &root_s])
            || succeeds(Path::new("unzip"), &["-oq", &zip_s, "-d", &root_s]));
    let _ = std::fs::remove_file(&zip);
    unpacked && root.join("platform-tools").is_dir()
}
