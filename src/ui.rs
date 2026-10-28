//! All terminal output and interactive input in one place, so the rest of the code never prints directly.

use crate::config;
use std::io::{self, BufRead, Write};

pub fn banner() {
    println!();
    println!("  {} - remove ads and junk, block trackers, speed up", config::APP_NAME);
    println!("  Everything it removes can be restored (Restore action)");
}

/// A new section header.
pub fn step(text: &str) {
    println!();
    println!("==> {text}");
}

pub fn ok(text: &str) {
    println!("  [OK] {text}");
}

pub fn warn(text: &str) {
    println!("  [!]  {text}");
}

pub fn fail(text: &str) {
    eprintln!("  [X]  {text}");
}

/// A plain indented line.
pub fn say(text: &str) {
    println!("  {text}");
}

/// A dim, secondary line (kept visually distinct from say() by an extra indent).
pub fn dim(text: &str) {
    println!("     {text}");
}

/// Read one line from stdin, trimmed. Returns an empty string on EOF.
pub fn read_line(prompt: &str) -> String {
    print!("  {prompt}: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => String::new(),
        Ok(_) => line.trim().to_string(),
    }
}

/// Yes/No question. `yes` is the default used for an empty answer (and always in non-interactive mode).
pub fn ask_yes_no(question: &str, default_yes: bool, assume_yes: bool) -> bool {
    if assume_yes {
        return default_yes;
    }
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    let a = read_line(&format!("{question} {hint}")).to_ascii_lowercase();
    if a.is_empty() {
        return default_yes;
    }
    matches!(a.as_str(), "y" | "yes")
}

/// Parse a selection like "1,3,5-7" into a set of 1-based indexes within 1..=max.
pub fn parse_numbers(text: &str, max: usize) -> std::collections::BTreeSet<usize> {
    let mut set = std::collections::BTreeSet::new();
    for part in text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(a), Ok(b)) = (a.trim().parse::<usize>(), b.trim().parse::<usize>()) {
                for i in a..=b {
                    if (1..=max).contains(&i) {
                        set.insert(i);
                    }
                }
            }
        } else if let Ok(i) = part.parse::<usize>() {
            if (1..=max).contains(&i) {
                set.insert(i);
            }
        }
    }
    set
}

/// Split items by 1-based numbers the person typed: (picked, the rest).
pub fn split_by_numbers<T: Clone>(items: &[T], numbers: &std::collections::BTreeSet<usize>) -> (Vec<T>, Vec<T>) {
    let mut picked = Vec::new();
    let mut rest = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if numbers.contains(&(i + 1)) {
            picked.push(item.clone());
        } else {
            rest.push(item.clone());
        }
    }
    (picked, rest)
}

/// Human-readable size from a megabyte count.
pub fn format_mb(mb: i64) -> String {
    if mb.abs() >= 1024 {
        format!("{:.1} GB", mb as f64 / 1024.0)
    } else {
        format!("{mb} MB")
    }
}

const BAR_WIDTH: usize = 24;

/// A text bar for a 0..1 fraction: [#########---------------].
pub fn bar(fraction: f64) -> String {
    let filled = (fraction.clamp(0.0, 1.0) * BAR_WIDTH as f64).round() as usize;
    format!("[{}{}]", "#".repeat(filled), "-".repeat(BAR_WIDTH - filled))
}

/// Numbered stages of a long run, each header showing how far along the whole run is.
pub struct Progress {
    total: usize,
    started: usize,
}

impl Progress {
    pub fn new(total: usize) -> Progress {
        Progress { total, started: 0 }
    }

    pub fn next(&mut self, title: &str) {
        let done = self.started.min(self.total);
        self.started += 1;
        let pct = 100 * done / self.total.max(1);
        step(&format!(
            "[{}/{}] {title}   {} {pct}% done",
            self.started,
            self.total,
            bar(done as f64 / self.total.max(1) as f64)
        ));
    }
}

/// A numbered list of "name - description" rows.
pub fn show_list(items: &[(String, String, bool)]) {
    for (i, (name, desc, enabled)) in items.iter().enumerate() {
        let state = if !*enabled { " (already disabled)" } else { "" };
        if desc.is_empty() {
            println!("  {:>3}. {name}{state}", i + 1);
        } else {
            println!("  {:>3}. {name} - {desc}{state}", i + 1);
        }
    }
}
