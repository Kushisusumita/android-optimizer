//! The command line, parsed once into `config::Opts`. `--flag value` and `--flag=value` both work.

use crate::config::{self, Action, Opts, TextSize};

const MIN_SCALE: f64 = 0.8;
const MAX_SCALE: f64 = 2.0;

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

Colours are off when the output is not a terminal or NO_COLOR is set.

{}",
        config::APP_NAME,
        config::APP_VERSION,
        config::DEFAULT_DNS_HOST,
        config::REPO_URL
    );
}
