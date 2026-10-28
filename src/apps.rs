//! Choosing which packages to remove: expand the lists against the installed set, then apply every
//! safety rule (protected, already disabled, excluded, kept, needs a replacement, installed by you).

use crate::config;
use crate::device::{DeviceInfo, Packages, Pkg};
use crate::lists::{self, PkgSpec, Section};
use crate::paths::Paths;
use crate::report;
use std::collections::HashSet;

pub struct Candidate {
    pub name: String,
    pub desc: String,
    pub section: Section,
    pub enabled: bool,
}

pub struct Candidates {
    pub items: Vec<Candidate>,
    /// Why matching packages were left alone.
    pub notes: Vec<String>,
}

impl Candidates {
    /// Ads and junk that should go (the [safe] section) - what the health score counts.
    pub fn junk_count(&self) -> usize {
        self.items.iter().filter(|c| c.section == Section::Safe).count()
    }
}

/// The person's own choices that override the lists.
pub struct Overrides<'a> {
    pub include: &'a [String],
    pub exclude: &'a HashSet<String>,
    pub keep: &'a HashSet<String>,
}

/// Malware first (so it wins ties and is attributed as malware), then manual includes, then
/// common/google/brand.
fn load_specs(paths: &Paths, info: &DeviceInfo, include: &[String]) -> Vec<PkgSpec> {
    let mut specs = lists::read_list(&paths.list_file("malware"));
    specs.extend(include.iter().map(|inc| PkgSpec {
        pattern: inc.clone(),
        section: Section::Safe,
        desc: "added manually".into(),
        needs: None,
        partner: false,
        media: false,
    }));
    for stem in info.list_stems() {
        specs.extend(lists::read_list(&paths.list_file(stem)));
    }
    specs
}

struct Rules<'a> {
    protected: Vec<String>,
    user_installers: Vec<&'static str>,
    pkgs: &'a Packages,
    overrides: &'a Overrides<'a>,
}

impl Rules<'_> {
    /// Why this package must be left alone, or None if it may be removed.
    fn rejection(&self, spec: &PkgSpec, pkg: &Pkg) -> Option<String> {
        let name = &pkg.name;
        if self.protected.iter().any(|p| lists::pattern_matches(p, name)) {
            return Some(format!("{name} - protected, not touched"));
        }
        if !pkg.enabled {
            return Some(format!("{name} - already disabled"));
        }
        if self.overrides.exclude.contains(name) {
            return Some(format!("{name} - excluded manually"));
        }
        if self.overrides.keep.contains(name) {
            return Some(format!("{name} - kept by your decision (keep)"));
        }
        if let Some(req) = spec.needs.as_ref().filter(|r| !self.pkgs.get(r).is_some_and(|a| a.enabled)) {
            return Some(format!("{name} - kept: no replacement ({req})"));
        }
        if spec.partner && !pkg.system && self.user_installers.contains(&pkg.installer.as_str()) {
            return Some(format!("{name} - installed by you ({}), not touched", pkg.installer));
        }
        None
    }
}

pub fn candidates(paths: &Paths, info: &DeviceInfo, pkgs: &Packages, overrides: &Overrides) -> Candidates {
    let mut user_installers = config::USER_INSTALLERS.to_vec();
    if !info.has_accounts {
        // With no account on the phone, nothing could have come from Play yet.
        user_installers.retain(|i| *i != "com.android.vending");
    }
    let rules =
        Rules { protected: lists::read_patterns(&paths.list_file("protected")), user_installers, pkgs, overrides };

    let mut out = Candidates { items: Vec::new(), notes: Vec::new() };
    let mut seen: HashSet<String> = HashSet::new();
    for spec in load_specs(paths, info, overrides.include) {
        for name in pkgs.matching(&spec.pattern) {
            let Some(pkg) = pkgs.get(&name).filter(|_| seen.insert(name.clone())) else { continue };
            match rules.rejection(&spec, pkg) {
                Some(note) => out.notes.push(note),
                None => out.items.push(Candidate {
                    name,
                    desc: spec.desc.clone(),
                    section: spec.section,
                    enabled: pkg.enabled,
                }),
            }
        }
    }
    out
}

/// Apps the person installed since this phone's previous run (from that run's package list).
pub fn new_since_last_run(paths: &Paths, device_id: &str, pkgs: &Packages) -> Vec<String> {
    let Some(last) =
        report::device_reports(paths, device_id).into_iter().rev().find(|d| d.join("packages-after.txt").is_file())
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
