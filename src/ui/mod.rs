//! All terminal output and questions in one place, so the rest of the code never prints directly.

pub mod chart;
mod prompt;
mod style;
#[cfg(test)]
mod tests;

pub use prompt::{ask_yes_no, parse_numbers, read_line, show_list, split_by_numbers};

use crate::config;
use style::{paint, Tone};

pub fn banner() {
    println!();
    println!(
        "  {}",
        paint(&format!("{} - remove ads and junk, block trackers, speed up", config::APP_NAME), Tone::Strong)
    );
    println!("  {}", paint("Everything it removes can be restored (Restore action)", Tone::Muted));
}

/// A new section header.
pub fn step(text: &str) {
    println!();
    println!("{} {}", paint("==>", Tone::Heading), paint(text, Tone::Strong));
}

pub fn ok(text: &str) {
    println!("  {} {text}", paint("[OK]", Tone::Good));
}

pub fn warn(text: &str) {
    println!("  {}  {text}", paint("[!]", Tone::Warning));
}

pub fn fail(text: &str) {
    eprintln!("  {}  {text}", paint("[X]", Tone::Error));
}

/// A plain indented line.
pub fn say(text: &str) {
    println!("  {text}");
}

/// A secondary line: indented further and greyed.
pub fn dim(text: &str) {
    println!("     {}", paint(text, Tone::Muted));
}

/// Human-readable size from a megabyte count.
pub fn format_mb(mb: i64) -> String {
    if mb.abs() >= 1024 {
        format!("{:.1} GB", mb as f64 / 1024.0)
    } else {
        format!("{mb} MB")
    }
}
