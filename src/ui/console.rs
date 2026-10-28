//! The terminal window itself: whether it understands colour codes, and whether it was opened only for this
//! program (a double-click, Win+R or a shortcut) - then Windows closes it the moment the program ends, so
//! the program waits for Enter first and the person can read what it said.

#[cfg(windows)]
mod win {
    pub type Handle = *mut std::ffi::c_void;
    pub const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    pub const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetStdHandle(which: u32) -> Handle;
        pub fn GetConsoleMode(console: Handle, mode: *mut u32) -> i32;
        pub fn SetConsoleMode(console: Handle, mode: u32) -> i32;
        pub fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
    }
}

/// Windows 10+ consoles understand colour codes once asked to; terminals that are not a Windows console
/// (Git Bash, Windows Terminal through a pipe) understand them already.
#[cfg(windows)]
pub fn accepts_colour() -> bool {
    use win::*;
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
pub fn accepts_colour() -> bool {
    std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
}

/// True when no other program (cmd, PowerShell, start.bat) shares this console window.
#[cfg(windows)]
pub fn opened_only_for_us() -> bool {
    let mut ids = [0u32; 4];
    // SAFETY: the buffer holds `ids.len()` process ids, as the call is told.
    let attached = unsafe { win::GetConsoleProcessList(ids.as_mut_ptr(), ids.len() as u32) };
    attached == 1
}

#[cfg(not(windows))]
pub fn opened_only_for_us() -> bool {
    false
}
