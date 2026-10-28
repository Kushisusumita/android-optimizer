//! Asking the person: a line of text, yes/no, or numbers picked from a list.

use super::style::{paint, Tone};
use std::collections::BTreeSet;
use std::io::{self, BufRead, Write};

/// Read one line from stdin, trimmed. Returns an empty string on EOF.
pub fn read_line(prompt: &str) -> String {
    print!("  {}: ", paint(prompt, Tone::Strong));
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => String::new(),
        Ok(_) => line.trim().to_string(),
    }
}

/// Yes/No question. `default_yes` answers an empty line - and every question when `assume_yes` is set.
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
pub fn parse_numbers(text: &str, max: usize) -> BTreeSet<usize> {
    let mut set = BTreeSet::new();
    for part in text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).map(str::trim) {
        let range = match part.split_once('-') {
            Some((a, b)) => a.trim().parse::<usize>().ok().zip(b.trim().parse::<usize>().ok()),
            None => part.parse::<usize>().ok().map(|i| (i, i)),
        };
        if let Some((a, b)) = range {
            set.extend((a..=b).filter(|i| (1..=max).contains(i)));
        }
    }
    set
}

/// Split items by 1-based numbers the person typed: (picked, the rest).
pub fn split_by_numbers<T: Clone>(items: &[T], numbers: &BTreeSet<usize>) -> (Vec<T>, Vec<T>) {
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

/// A numbered list of "name - description" rows.
pub fn show_list(items: &[(String, String, bool)]) {
    for (i, (name, desc, enabled)) in items.iter().enumerate() {
        let number = paint(&format!("{:>3}.", i + 1), Tone::Muted);
        let state = if *enabled { String::new() } else { paint(" (already disabled)", Tone::Muted) };
        let desc = if desc.is_empty() { String::new() } else { paint(&format!(" - {desc}"), Tone::Muted) };
        println!("  {number} {name}{desc}{state}");
    }
}
