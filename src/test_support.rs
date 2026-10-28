//! Helpers shared by the tests of every layer.

use crate::host::paths::Paths;
use crate::phone::brands::Props;
use crate::phone::info::DeviceInfo;
use std::fs;
use std::path::PathBuf;

pub fn props(pairs: &[(&str, &str)]) -> Props {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

pub fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// One fixed folder per test, emptied at the start of each run, so runs do not pile up folders.
pub fn temp_root(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("android-optimizer-tests").join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn temp_paths(name: &str) -> Paths {
    Paths::locate(Some(temp_root(name).join("lists").to_str().unwrap())).unwrap()
}

pub fn repo_paths() -> Paths {
    Paths::locate(Some(concat!(env!("CARGO_MANIFEST_DIR"), "/lists"))).unwrap()
}

pub fn xiaomi(has_accounts: bool) -> DeviceInfo {
    let p = props(&[
        ("ro.product.brand", "Redmi"),
        ("ro.product.manufacturer", "Xiaomi"),
        ("ro.product.model", "TEST-MODEL"),
        ("ro.serialno", "TESTSERIAL"),
    ]);
    DeviceInfo::from_props(&p, "usb-serial", has_accounts)
}
