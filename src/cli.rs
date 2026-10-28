//! The command line: parse the arguments once, connect to the phone, ask every question up front,
//! then run the chosen action unattended - with stage progress, the health score and a final chart.

use crate::adb::Adb;
use crate::adfinder;
use crate::apps::{self, Candidate, Candidates, Overrides};
use crate::brands::Brand;
use crate::config::{self, Action, TextSize};
use crate::device::{DeviceInfo, Packages, Phone};
use crate::diagnostics::{self, Diag};
use crate::hardware;
use crate::lists::{self, Section};
use crate::paths::Paths;
use crate::platform_tools;
use crate::report::{self, Report};
use crate::restore;
use crate::root::{Probe, Root};
use crate::score::{self, Measure};
use crate::security;
use crate::tweaks::{self, Plan, Tweaks};
use crate::ui;
use crate::usb::{self, UsbPhone};
use std::collections::HashSet;

const NO_LISTS: &str = "The lists/ folder was not found. Run from the project folder or pass --lists <folder>.";
const NO_ADB: &str =
    "adb was not found. Install Android platform-tools (or put it in ./platform-tools) or pass --adb <path>.";
const MIN_SCALE: f64 = 0.8;
const MAX_SCALE: f64 = 2.0;

#[derive(Default)]
pub struct Opts {
    pub action: Option<Action>,
    pub yes: bool,
    pub help: bool,
    pub version: bool,
    pub serial: Option<String>,
    pub connect: Option<String>,
    pub pair: Option<String>,
    pub pair_code: Option<String>,
    pub exclude: Vec<String>,
    pub include: Vec<String>,
    pub background: Vec<String>,
    pub mute: Vec<String>,
    pub text_size: Option<TextSize>,
    pub font_scale: Option<f64>,
    pub display_scale: Option<f64>,
    pub no_dns: bool,
    pub no_compile: bool,
    pub reboot: bool,
    pub dns_host: Option<String>,
    pub lists_dir: Option<String>,
    pub adb_path: Option<String>,
}

type Switch = fn(&mut Opts);
type Valued = fn(&mut Opts, String) -> Result<(), String>;

const SWITCHES: &[(&str, Switch)] = &[
    ("-h", |o| o.help = true),
    ("--help", |o| o.help = true),
    ("--version", |o| o.version = true),
    ("-y", |o| o.yes = true),
    ("--yes", |o| o.yes = true),
    ("--no-dns", |o| o.no_dns = true),
    ("--no-compile", |o| o.no_compile = true),
    ("--reboot", |o| o.reboot = true),
];

const VALUED: &[(&str, Valued)] = &[
    ("--serial", |o, v| {
        o.serial = Some(v);
        Ok(())
    }),
    ("--connect", |o, v| {
        o.connect = Some(v);
        Ok(())
    }),
    ("--pair", |o, v| {
        o.pair = Some(v);
        Ok(())
    }),
    ("--code", |o, v| {
        o.pair_code = Some(v);
        Ok(())
    }),
    ("--exclude", |o, v| {
        o.exclude.extend(split_list(&v));
        Ok(())
    }),
    ("--include", |o, v| {
        o.include.extend(split_list(&v));
        Ok(())
    }),
    ("--background", |o, v| {
        o.background.extend(split_list(&v));
        Ok(())
    }),
    ("--mute", |o, v| {
        o.mute.extend(split_list(&v));
        Ok(())
    }),
    ("--text-size", |o, v| {
        o.text_size = Some(TextSize::parse(&v).ok_or(format!("unknown text size: {v}"))?);
        Ok(())
    }),
    ("--font-scale", |o, v| {
        o.font_scale = Some(parse_scale(&v)?);
        Ok(())
    }),
    ("--display-scale", |o, v| {
        o.display_scale = Some(parse_scale(&v)?);
        Ok(())
    }),
    ("--dns", |o, v| {
        o.dns_host = Some(v);
        Ok(())
    }),
    ("--lists", |o, v| {
        o.lists_dir = Some(v);
        Ok(())
    }),
    ("--adb", |o, v| {
        o.adb_path = Some(v);
        Ok(())
    }),
];

fn split_list(v: &str) -> Vec<String> {
    v.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

fn parse_scale(v: &str) -> Result<f64, String> {
    v.trim()
        .parse::<f64>()
        .ok()
        .filter(|x| (MIN_SCALE..=MAX_SCALE).contains(x))
        .ok_or(format!("scale must be a number from {MIN_SCALE} to {MAX_SCALE}: {v}"))
}

/// Parse the command line once. `--flag value` and `--flag=value` are both accepted.
pub fn parse_args<I: IntoIterator<Item = String>>(args: I) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        if let Some((_, set)) = SWITCHES.iter().find(|(f, _)| *f == flag) {
            set(&mut o);
        } else if let Some((_, set)) = VALUED.iter().find(|(f, _)| *f == flag) {
            let value = inline.or_else(|| it.next()).ok_or(format!("{flag} needs a value"))?;
            set(&mut o, value)?;
        } else if flag.starts_with('-') {
            return Err(format!("unknown option: {flag}"));
        } else {
            o.action = Some(Action::parse(&flag).ok_or(format!("unknown action: {flag}"))?);
        }
    }
    Ok(o)
}

pub fn usage() {
    println!(
        "{} {}

Usage: android-optimizer [action] [options]

Actions (without one, a menu asks):
  optimize   remove ads and junk, block trackers, speed up (recommended)
  max        also remove optional apps (you choose)
  speedup    clean caches and compile apps only
  security   look for viruses and spyware (changes nothing)
  adsource   find the app behind pop-up ads (changes nothing)
  check      show what would change (changes nothing)
  restore    bring back removed apps and the original settings

Options:
  -y, --yes                 no questions: safe defaults
  --serial <id>             pick a phone when several are connected
  --pair <ip:port> --code <code>   pair over Wi-Fi (Android 11+)
  --connect <ip:port>       connect over Wi-Fi
  --exclude <a,b>           never remove these packages
  --include <a,b>           also remove these packages
  --background <a,b>        limit background activity for these apps
  --mute <a,b>              turn off notifications for these apps
  --text-size <0-3>         0 unchanged, 1 large, 2 larger, 3 largest
  --font-scale <x>          exact font scale ({MIN_SCALE}-{MAX_SCALE})
  --display-scale <x>       exact display size factor ({MIN_SCALE}-{MAX_SCALE})
  --dns <host>              Private DNS for ad blocking (default {})
  --no-dns                  do not change Private DNS
  --no-compile              skip app compilation
  --reboot                  restart the phone at the end
  --lists <folder>          use another lists folder
  --adb <path>              use a specific adb executable

{}",
        config::APP_NAME,
        config::APP_VERSION,
        config::DEFAULT_DNS_HOST,
        config::REPO_URL
    );
}

/// The program: returns the process exit code.
pub fn run() -> i32 {
    let opts = match parse_args(std::env::args().skip(1)) {
        Ok(o) => o,
        Err(e) => {
            ui::fail(&e);
            ui::say("Run with --help to see the options.");
            return 2;
        }
    };
    if opts.help || opts.version {
        if opts.help {
            usage()
        } else {
            println!("{} {}", config::APP_NAME, config::APP_VERSION)
        }
        return 0;
    }
    ui::banner();
    match start(opts) {
        Ok(()) => 0,
        Err(e) => {
            ui::fail(&e);
            1
        }
    }
}

struct Session {
    phone: Phone,
    paths: Paths,
    opts: Opts,
    report: Report,
}

fn start(opts: Opts) -> Result<(), String> {
    let paths = Paths::locate(opts.lists_dir.as_deref()).ok_or(NO_LISTS)?;
    let adb = match Adb::locate(opts.adb_path.as_deref(), Some(&paths.root)) {
        Some(adb) => adb,
        None => download_adb(&paths, &opts)?,
    };
    adb.start_server();
    let action = choose_action(&opts);
    let phone = connect(adb, &opts)?;
    introduce(&phone.info);
    warn_if_updated(&paths, &phone.info);
    let report = Report::create(&paths, &phone.info, action.tag());
    dispatch(action, &Session { phone, paths, opts, report });
    Ok(())
}

fn download_adb(paths: &Paths, opts: &Opts) -> Result<Adb, String> {
    ui::warn("adb (Android platform-tools) is not installed on this computer.");
    if !ui::ask_yes_no("Download Google's official platform-tools (about 7 MB) into this folder?", true, opts.yes) {
        return Err(NO_ADB.into());
    }
    ui::say(&format!("Downloading {}", platform_tools::download_url()));
    if !platform_tools::install(&paths.root) {
        return Err(format!("The download failed. Check the internet connection. {NO_ADB}"));
    }
    ui::ok("platform-tools installed");
    Adb::locate(None, Some(&paths.root)).ok_or_else(|| NO_ADB.into())
}

const MENU: &[(Action, &str)] = &[
    (Action::Optimize, "Optimize - remove ads and junk, block trackers, speed up (recommended)"),
    (Action::Max, "Maximum - also remove optional apps (you choose)"),
    (Action::SpeedUp, "Speed up only - clean caches and compile apps"),
    (Action::Security, "Security check - look for viruses and spyware"),
    (Action::AdSource, "Find the source of pop-up ads"),
    (Action::Check, "Check only - show what would change, change nothing"),
    (Action::Restore, "Restore - bring back removed apps and settings"),
];

fn choose_action(opts: &Opts) -> Action {
    if let Some(a) = opts.action {
        return a;
    }
    if opts.yes {
        return Action::Optimize;
    }
    ui::step("What should we do?");
    for (i, (_, text)) in MENU.iter().enumerate() {
        ui::say(&format!("{}. {text}", i + 1));
    }
    let n = ui::read_line("Your choice (Enter = 1)").parse::<usize>().ok().filter(|n| (1..=MENU.len()).contains(n));
    MENU[n.unwrap_or(1) - 1].0
}

/// Brand-exact connection steps that update as soon as a phone shows up on the USB bus.
#[derive(Default)]
struct ConnectGuide {
    polls: u32,
    shown: Option<Option<Brand>>,
}

impl ConnectGuide {
    fn refresh(&mut self) {
        let due = self.polls.is_multiple_of(config::USB_SCAN_EVERY_POLLS);
        self.polls += 1;
        if !due {
            return;
        }
        let found = usb::find_phone();
        let key = found.as_ref().map(|p| p.brand);
        if self.shown != Some(key) {
            self.shown = Some(key);
            show_connect_steps(found);
        }
    }
}

fn show_connect_steps(found: Option<UsbPhone>) {
    let brand = match &found {
        Some(p) => {
            ui::step(&format!("Found on USB: {} ({}). Now turn on USB debugging:", p.name, p.brand.title()));
            p.brand
        }
        None => {
            ui::step("Connect the phone with a USB data cable (some cables only charge)");
            ui::dim("The exact steps for your phone's brand appear as soon as it is plugged in.");
            Brand::Other
        }
    };
    for line in brand.connect_steps() {
        ui::say(&line);
    }
    ui::dim("No working cable? Android 11+: Developer options -> Wireless debugging -> Pair device with pairing code,");
    ui::dim("then start again with: --pair <ip:port> --code <code>");
}

fn connect(mut adb: Adb, opts: &Opts) -> Result<Phone, String> {
    let wireless = (opts.pair.is_some() || opts.connect.is_some())
        .then(|| adb.connect_wifi(opts.pair.as_deref(), opts.pair_code.as_deref(), opts.connect.as_deref()))
        .flatten();
    let wanted = wireless.or_else(|| opts.serial.clone());
    let mut guide = ConnectGuide::default();
    let serial = adb.wait_for_device(wanted.as_deref(), || guide.refresh()).ok_or("No phone connected.")?;
    adb.set_serial(serial);
    let info = DeviceInfo::load(&adb);
    let root = detect_root(&adb);
    Ok(Phone { adb, info, root })
}

/// Silent unless root is really there: phones without root see nothing about it.
fn detect_root(adb: &Adb) -> Option<Root> {
    match Root::probe(adb) {
        Probe::Ready(root) => {
            ui::ok("Root found - extra read-only checks are on (CPU limits, GPU load, battery wear)");
            Some(root)
        }
        Probe::Denied => {
            ui::dim(
                "Root is installed, but Shell has no access - allow \"Shell\" in Magisk/KernelSU for extra checks.",
            );
            None
        }
        Probe::Absent => None,
    }
}

fn introduce(info: &DeviceInfo) {
    ui::step(&format!("Phone: {} ({})", info.name, info.model));
    ui::say(&format!("{} - Android {} (API {}), {}", info.brand.title(), info.android, info.sdk, info.rom));
}

fn warn_if_updated(paths: &Paths, info: &DeviceInfo) {
    let previous = report::last_fingerprint(paths, &info.device_id);
    if previous.is_some_and(|fp| !fp.is_empty() && fp != info.fingerprint) {
        ui::warn("The system was updated since the last run - updates often bring removed apps back.");
    }
}

struct Snapshot {
    pkgs: Packages,
    diag: Diag,
    cands: Candidates,
}

fn find_candidates(s: &Session, pkgs: &Packages) -> Candidates {
    let keep: HashSet<String> = lists::read_patterns(&s.paths.keep_file(&s.phone.info.device_id)).into_iter().collect();
    let exclude: HashSet<String> = s.opts.exclude.iter().cloned().collect();
    let overrides = Overrides { include: &s.opts.include, exclude: &exclude, keep: &keep };
    apps::candidates(&s.paths, &s.phone.info, pkgs, &overrides)
}

fn snapshot(s: &Session) -> Snapshot {
    let pkgs = Packages::load(&s.phone.adb);
    let diag = diagnostics::collect(&s.phone, &pkgs);
    let cands = find_candidates(s, &pkgs);
    Snapshot { pkgs, diag, cands }
}

fn dispatch(action: Action, s: &Session) {
    if action == Action::Restore {
        restore::run(&s.phone, &s.paths, s.opts.yes);
        return;
    }
    let before = snapshot(s);
    s.report.write_packages("packages-before.txt", &before.pkgs);
    show_state(s, &before);
    match action {
        Action::AdSource => adfinder::find(&s.phone.adb, &before.pkgs),
        Action::SpeedUp => speed_up(s, &before),
        _ => {
            security::run(&s.phone, &before.pkgs, &s.paths, s.report.path());
            if action != Action::Security {
                optimize(action, s, &before);
            }
        }
    }
}

fn show_state(s: &Session, snap: &Snapshot) {
    ui::step("Phone health");
    diagnostics::show(&snap.diag);
    diagnostics::warnings(&s.phone.adb, &snap.diag, &s.phone.info);
    hardware::show_specs(&s.phone, &snap.diag);
    hardware::show_load(&s.phone, &snap.diag);
    let points = score::score(&snap.diag, snap.cands.junk_count());
    ui::step(&format!("Health score: {points}/100 ({})", score::label(points)));
}

fn measure_speed(s: &Session) -> Option<i64> {
    ui::step("Measuring app start speed (Settings will open and close a few times)");
    let ms = hardware::launch_ms(&s.phone.adb);
    if let Some(ms) = ms {
        ui::say(&format!("Settings opens in {ms} ms"));
    }
    ms
}

fn speed_up(s: &Session, before: &Snapshot) {
    let launch = measure_speed(s);
    let mut progress = ui::Progress::new(3);
    progress.next("Cleaning caches");
    tweaks::clean_caches(&s.phone.adb);
    progress.next("Compiling apps");
    compile_unless_skipped(s, &before.diag);
    progress.next("Measuring the result");
    finish(s, before, launch);
}

fn compile_unless_skipped(s: &Session, diag: &Diag) {
    if s.opts.no_compile {
        ui::dim("Skipped (--no-compile)");
    } else {
        tweaks::optimize_apps(&s.phone.adb, &s.phone.info, diag);
    }
    tweaks::stop_background(&s.phone.adb);
}

fn optimize(action: Action, s: &Session, before: &Snapshot) {
    let shown: Vec<&Candidate> =
        before.cands.items.iter().filter(|c| action == Action::Max || c.section == Section::Safe).collect();
    show_new_apps(s, &before.pkgs);
    s.report.write("skipped.txt", &before.cands.notes.join("\n"));
    if action == Action::Check {
        preview(&shown, &before.cands.notes);
        return;
    }
    // Every question first; after this the run needs nobody at the keyboard.
    let chosen = select_removals(s, &shown);
    let plan =
        tweaks::request_settings_access(&s.phone.adb, &s.phone.info, s.opts.yes).then(|| choose_plan(s, &before.pkgs));
    let launch = measure_speed(s);

    let mut progress = ui::Progress::new(5);
    progress.next("Removing ads and junk");
    let removed = tweaks::remove_packages(&s.phone.adb, &s.report, &chosen);
    show_interim(before, removed);
    progress.next("Settings, ad blocking and limits");
    match &plan {
        Some(plan) => apply_settings(s, plan),
        None => ui::dim("Skipped: the phone does not allow changing settings"),
    }
    progress.next("Cleaning caches");
    tweaks::clean_caches(&s.phone.adb);
    progress.next("Compiling apps");
    compile_unless_skipped(s, &before.diag);
    progress.next("Measuring the result");
    finish(s, before, launch);
}

fn show_new_apps(s: &Session, pkgs: &Packages) {
    let fresh = apps::new_since_last_run(&s.paths, &s.phone.info.device_id, pkgs);
    if !fresh.is_empty() {
        ui::step("Installed since the last run");
        for n in &fresh {
            ui::say(n);
        }
    }
}

fn rows(items: &[&Candidate]) -> Vec<(String, String, bool)> {
    items.iter().map(|c| (c.name.clone(), c.desc.clone(), c.enabled)).collect()
}

fn preview(shown: &[&Candidate], notes: &[String]) {
    ui::step(&format!("Would remove {} apps (Check changes nothing)", shown.len()));
    ui::show_list(&rows(shown));
    if !notes.is_empty() {
        ui::dim("Left alone:");
        for n in notes {
            ui::dim(n);
        }
    }
}

/// The removals the person agreed to, as (package, description). Kept apps are remembered for this phone.
fn select_removals(s: &Session, shown: &[&Candidate]) -> Vec<(String, String)> {
    if shown.is_empty() {
        ui::ok("No ads or junk found");
        return Vec::new();
    }
    ui::step(&format!("Ads and junk to remove: {}", shown.len()));
    ui::show_list(&rows(shown));
    let answer = if s.opts.yes {
        String::new()
    } else {
        ui::say("Enter - remove all; numbers (e.g. 2,5) - keep those apps (not offered again); 0 - remove nothing");
        ui::read_line("Your choice")
    };
    if answer == "0" {
        return Vec::new();
    }
    let (kept, removed) = ui::split_by_numbers(shown, &ui::parse_numbers(&answer, shown.len()));
    let kept: Vec<String> = kept.iter().map(|c| c.name.clone()).collect();
    lists::add_keep(&s.paths.keep_file(&s.phone.info.device_id), &kept);
    removed.iter().map(|c| (c.name.clone(), c.desc.clone())).collect()
}

fn choose_plan(s: &Session, pkgs: &Packages) -> Plan {
    let (font, display) = text_scales(s);
    Plan {
        font,
        display,
        background: choose_apps(
            s,
            pkgs,
            &s.opts.background,
            "Limit background activity (saves battery and RAM) for which apps?",
        ),
        mute: choose_apps(s, pkgs, &s.opts.mute, "Turn off notifications for which apps?"),
    }
}

fn text_scales(s: &Session) -> (f64, f64) {
    let explicit = s.opts.font_scale.is_some() || s.opts.display_scale.is_some();
    let preset = match s.opts.text_size {
        Some(t) => t,
        None if s.opts.yes || explicit => TextSize::Unchanged,
        None => ask_text_size(),
    };
    let (font, display) = preset.scales();
    (s.opts.font_scale.unwrap_or(font), s.opts.display_scale.unwrap_or(display))
}

fn ask_text_size() -> TextSize {
    ui::step("Bigger text? (helps with poor eyesight)");
    ui::say("0 - leave as is   1 - large   2 - larger   3 - largest");
    TextSize::parse(&ui::read_line("Your choice (Enter = 0)")).unwrap_or(TextSize::Unchanged)
}

/// Apps from lists/background.txt that are installed, offered as a numbered list. Names given on the
/// command line are checked here, once: only installed ones go on.
fn choose_apps(s: &Session, pkgs: &Packages, preset: &[String], question: &str) -> Vec<String> {
    if !preset.is_empty() {
        return preset.iter().filter(|p| pkgs.contains(p)).cloned().collect();
    }
    if s.opts.yes {
        return Vec::new();
    }
    let offer: Vec<(String, String, bool)> = lists::read_list(&s.paths.list_file("background"))
        .iter()
        .flat_map(|spec| pkgs.matching(&spec.pattern).into_iter().map(|n| (n, spec.desc.clone(), true)))
        .collect();
    if offer.is_empty() {
        return Vec::new();
    }
    ui::step(question);
    ui::show_list(&offer);
    let numbers = ui::parse_numbers(&ui::read_line("Numbers, e.g. 1,3 (Enter = none)"), offer.len());
    ui::split_by_numbers(&offer, &numbers).0.into_iter().map(|(n, _, _)| n).collect()
}

fn apply_settings(s: &Session, plan: &Plan) {
    let mut tw = Tweaks::new(&s.phone.adb, &s.report);
    tweaks::apply_tweaks(&mut tw, &s.phone.info, plan, &s.paths);
    if !s.opts.no_dns {
        tweaks::enable_adblock_dns(
            &mut tw,
            &s.phone.info,
            s.opts.dns_host.as_deref().unwrap_or(config::DEFAULT_DNS_HOST),
        );
    }
    if tw.secure_denied {
        ui::warn("The phone refused some changes:");
        for line in s.phone.info.brand.setup_help() {
            ui::dim(line);
        }
    }
}

fn show_interim(before: &Snapshot, removed: usize) {
    let junk = before.cands.junk_count();
    let was = score::score(&before.diag, junk);
    let now = score::score(&before.diag, junk.saturating_sub(removed));
    ui::say(&format!("Health score so far: {was} -> {now}"));
}

fn finish(s: &Session, before: &Snapshot, launch_before: Option<i64>) {
    let after = snapshot(s);
    s.report.write_packages("packages-after.txt", &after.pkgs);
    let launch_after = launch_before.and_then(|_| hardware::launch_ms(&s.phone.adb));
    score::show_chart(
        &Measure::new(&before.diag, before.cands.junk_count(), launch_before),
        &Measure::new(&after.diag, after.cands.junk_count(), launch_after),
    );
    ui::dim(&format!("Report (used by Restore): {}", s.report.path().display()));
    maybe_reboot(s, &after.diag);
}

fn maybe_reboot(s: &Session, diag: &Diag) {
    if diag.stuck_volume_key.is_some() {
        if s.opts.reboot {
            ui::warn("Not restarting: a volume key looks stuck, and the phone would boot into recovery.");
        }
        return;
    }
    if s.opts.reboot || ui::ask_yes_no("Restart the phone now to finish (recommended)?", false, s.opts.yes) {
        s.phone.adb.reboot();
        ui::ok("Restarting the phone");
    }
}
