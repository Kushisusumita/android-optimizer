//! Reading and tapping the phone's screen by on-screen text (`uiautomator dump`), never by guessed
//! coordinates. The step-by-step flows that use it live in `tune::screen_flows`.

use super::adb::Adb;
use super::info::parse_wm_size;
use std::thread::sleep;
use std::time::Duration;

pub struct UiNode {
    pub text: String,
    pub resource_id: String,
    pub class: String,
    pub checked: bool,
    pub bounds: Option<(i32, i32, i32, i32)>,
}

impl UiNode {
    pub fn center(&self) -> Option<(i32, i32)> {
        self.bounds.map(|(x1, y1, x2, y2)| ((x1 + x2) / 2, (y1 + y2) / 2))
    }
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

fn attr(node: &str, name: &str) -> String {
    let pat = format!("{name}=\"");
    if let Some(i) = node.find(&pat) {
        let rest = &node[i + pat.len()..];
        if let Some(j) = rest.find('"') {
            return unescape(&rest[..j]);
        }
    }
    String::new()
}

fn parse_bounds(s: &str) -> Option<(i32, i32, i32, i32)> {
    // Format: [x1,y1][x2,y2]
    let s = s.replace("][", ",");
    let s = s.trim_matches(|c| c == '[' || c == ']');
    let n: Vec<i32> = s.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    if n.len() == 4 {
        Some((n[0], n[1], n[2], n[3]))
    } else {
        None
    }
}

pub fn dump_nodes(adb: &Adb) -> Vec<UiNode> {
    let _ = adb.sh("uiautomator dump /sdcard/clearphone_ui.xml");
    let xml = adb.pull_text("/sdcard/clearphone_ui.xml");
    let _ = adb.sh("rm -f /sdcard/clearphone_ui.xml");
    let mut nodes = Vec::new();
    for part in xml.split("<node ").skip(1) {
        let node = part.split('>').next().unwrap_or("");
        nodes.push(UiNode {
            text: attr(node, "text"),
            resource_id: attr(node, "resource-id"),
            class: attr(node, "class"),
            checked: attr(node, "checked") == "true",
            bounds: parse_bounds(&attr(node, "bounds")),
        });
    }
    nodes
}

pub fn tap(adb: &Adb, x: i32, y: i32) {
    let _ = adb.sh(&format!("input tap {x} {y}"));
}

pub fn tap_node(adb: &Adb, node: &UiNode) -> bool {
    if let Some((x, y)) = node.center() {
        tap(adb, x, y);
        true
    } else {
        false
    }
}

pub fn scroll(adb: &Adb) {
    let (w, h) = parse_wm_size(&adb.sh("wm size")).unwrap_or((1080, 2400));
    let x = w / 2;
    let _ = adb.sh(&format!("input swipe {x} {} {x} {} 400", h * 8 / 10, h * 4 / 10));
    sleep(Duration::from_millis(1500));
}
