//! Colours for the terminal. On only when the output is a terminal and NO_COLOR is not set, so files,
//! pipes and AI assistants reading the output get plain text.

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
        std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal() && terminal_accepts_colour()
    })
}

/// Windows 10+ consoles understand colour codes once asked to; terminals that are not a Windows console
/// (Git Bash, Windows Terminal through a pipe) understand them already.
#[cfg(windows)]
fn terminal_accepts_colour() -> bool {
    type Handle = *mut std::ffi::c_void;
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> Handle;
        fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
        fn SetConsoleMode(console: Handle, mode: u32) -> i32;
    }
    // SAFETY: Win32 calls on this process's own stdout handle; `mode` lives across the call.
    unsafe {
        let out = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if GetConsoleMode(out, &mut mode) == 0 {
            return true;
        }
        mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0
            || SetConsoleMode(out, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
    }
}

#[cfg(not(windows))]
fn terminal_accepts_colour() -> bool {
    std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
}
