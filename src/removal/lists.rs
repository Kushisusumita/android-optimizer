//! Parsing the package lists in `lists/*.txt` and the per-phone keep list.
//!
//! Line format:  package.name  # description  [@needs replacement] [@partner] [@media]
//! Sections:     [safe] (removed by Optimize)   [max] (removed by Maximum)

use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Safe,
    Max,
}

#[derive(Clone, Debug)]
pub struct PkgSpec {
    pub pattern: String,
    pub section: Section,
    pub desc: String,
    /// Removed only if this replacement app is installed and enabled.
    pub needs: Option<String>,
    /// A partner app: kept when the person installed it themselves.
    pub partner: bool,
    /// Plays media in the background - never put to sleep by the Xiaomi background limit.
    pub media: bool,
}

/// (code, comment) of a line.
fn split_comment(line: &str) -> (&str, &str) {
    match line.split_once('#') {
        Some((code, comment)) => (code.trim(), comment.trim()),
        None => (line.trim(), ""),
    }
}

fn section_header(s: &str) -> Option<Section> {
    let inner = s.trim().strip_prefix('[')?.strip_suffix(']')?;
    Some(if inner.eq_ignore_ascii_case("max") { Section::Max } else { Section::Safe })
}

fn parse_spec(code: &str, comment: &str, section: Section) -> PkgSpec {
    let needs = comment.split_whitespace().skip_while(|t| *t != "@needs").nth(1).map(String::from);
    let mut desc = comment.to_string();
    if let Some(n) = &needs {
        desc = desc.replace(&format!("@needs {n}"), "");
    }
    PkgSpec {
        pattern: code.to_string(),
        section,
        desc: desc.replace("@partner", "").replace("@media", "").trim().to_string(),
        needs,
        partner: comment.contains("@partner"),
        media: comment.contains("@media"),
    }
}

/// Parse the text of a list file into package specs (with sections and flags).
pub fn parse_list(text: &str) -> Vec<PkgSpec> {
    let mut out = Vec::new();
    let mut section = Section::Safe;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if let Some(sec) = section_header(line) {
            section = sec;
            continue;
        }
        let (code, comment) = split_comment(line);
        if !code.is_empty() {
            out.push(parse_spec(code, comment, section));
        }
    }
    out
}

pub fn read_list(path: &Path) -> Vec<PkgSpec> {
    parse_list(&fs::read_to_string(path).unwrap_or_default())
}

/// Plain patterns of a file (protected.txt, malware.txt, keep files), without [section] headers.
pub fn parse_patterns(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| split_comment(l).0)
        .filter(|c| !c.is_empty() && section_header(c).is_none())
        .map(String::from)
        .collect()
}

pub fn read_patterns(path: &Path) -> Vec<String> {
    parse_patterns(&fs::read_to_string(path).unwrap_or_default())
}

/// Append package names to a per-phone keep file (deduplicated).
pub fn add_keep(path: &Path, names: &[String]) {
    let mut known = read_patterns(path);
    let mut fresh = Vec::new();
    for n in names.iter().filter(|n| !n.is_empty()) {
        if !known.contains(n) {
            known.push(n.clone());
            fresh.push(n);
        }
    }
    if fresh.is_empty() {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        for n in fresh {
            let _ = writeln!(f, "{n}");
        }
    }
}
