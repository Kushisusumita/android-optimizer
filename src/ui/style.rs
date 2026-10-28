//! Colours for the terminal. On only when the output is a terminal and NO_COLOR is not set, so files,
//! pipes and AI assistants reading the output get plain text.

use super::console;
use std::io::IsTerminal;
use std::sync::OnceLock;

#[derive(Clone, Copy)]
pub enum Tone {
    Heading,
    Strong,
    Good,
    Warning,
    Error,
    Muted,
    /// The "before" bar of a chart.
    Before,
    /// The "after" bar of a chart.
    After,
}

impl Tone {
    fn code(self) -> &'static str {
        match self {
            Tone::Heading => "1;36",
            Tone::Strong => "1",
            Tone::Good | Tone::After => "32",
            Tone::Warning | Tone::Before => "33",
            Tone::Error => "1;31",
            Tone::Muted => "90",
        }
    }
}

pub fn paint(text: &str, tone: Tone) -> String {
    if enabled() {
        format!("\x1b[{}m{text}\x1b[0m", tone.code())
    } else {
        text.to_string()
    }
}

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal() && console::accepts_colour()
    })
}
