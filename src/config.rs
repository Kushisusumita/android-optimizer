//! Constants and runtime options - every tunable value lives here, nothing is hard-coded elsewhere.

pub const APP_NAME: &str = "Android Optimizer";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO_URL: &str = "https://github.com/Kushisusumita/android-optimizer";

/// Private DNS host used for system-wide ad/tracker blocking (DNS-over-TLS filter).
pub const DEFAULT_DNS_HOST: &str = "dns.adguard-dns.com";

/// Animation scale applied for a snappier UI (0.5 = twice as fast).
pub const ANIMATION_SCALE: &str = "0.5";

/// Text-size presets: (font_scale, display_size_factor). Display size grows text, buttons and icons
/// together so layouts keep fitting; a huge font alone pushes buttons off screen.
pub const TEXT_LARGE: (f64, f64) = (1.10, 1.05);
pub const TEXT_LARGER: (f64, f64) = (1.15, 1.10);
pub const TEXT_LARGEST: (f64, f64) = (1.15, 1.20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextSize {
    Unchanged,
    Large,
    Larger,
    Largest,
}

impl TextSize {
    /// Accepts the name or the menu number (0-3).
    pub fn parse(s: &str) -> Option<TextSize> {
        match s.trim().to_ascii_lowercase().as_str() {
            "0" | "normal" | "unchanged" | "" => Some(TextSize::Unchanged),
            "1" | "large" => Some(TextSize::Large),
            "2" | "larger" => Some(TextSize::Larger),
            "3" | "largest" => Some(TextSize::Largest),
            _ => None,
        }
    }

    /// (font_scale, display_size_factor); 0.0 means "leave as is".
    pub fn scales(self) -> (f64, f64) {
        match self {
            TextSize::Unchanged => (0.0, 0.0),
            TextSize::Large => TEXT_LARGE,
            TextSize::Larger => TEXT_LARGER,
            TextSize::Largest => TEXT_LARGEST,
        }
    }
}

/// Never shrink the layout below this logical width (dp) when scaling the display size.
pub const MIN_SMALLEST_WIDTH_DP: f64 = 320.0;

/// Full compilation needs real free space (compiled code is several GB). Require at least this much,
/// and at least this fraction of total storage, free.
pub const COMPILE_MIN_FREE_MB: i64 = 8192;
pub const COMPILE_MIN_FREE_FRACTION: f64 = 0.25;

/// Seconds to wait for a `su` probe before giving up (read-only root detection).
pub const ROOT_PROBE_SECS: u64 = 5;

/// How long to wait for a phone to appear, and how often to look.
pub const DEVICE_WAIT_SECS: u64 = 15 * 60;
pub const DEVICE_POLL_SECS: u64 = 2;
/// Look at the computer's USB device list every N polls while waiting (it is slower than `adb devices`).
pub const USB_SCAN_EVERY_POLLS: u32 = 3;

/// Stores and installers people use themselves. Partner apps (@partner) installed this way are kept.
pub const USER_INSTALLERS: &[&str] = &[
    "com.android.vending",
    "com.google.android.packageinstaller",
    "com.android.packageinstaller",
    "com.miui.packageinstaller",
    "com.samsung.android.packageinstaller",
    "com.sec.android.app.samsungapps",
    "com.huawei.appmarket",
    "ru.vk.store",
    "com.android.shell",
    "org.fdroid.fdroid",
    "com.aurora.store",
];

/// Package installers that mean "installed from an APK file", not from a store (a malware signal).
pub const APK_INSTALLERS: &[&str] = &[
    "com.google.android.packageinstaller",
    "com.android.packageinstaller",
    "com.miui.packageinstaller",
    "com.samsung.android.packageinstaller",
];

/// Everything the person asked for on the command line, parsed once (see `cli::args`).
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

/// What the user asked the tool to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Optimize,
    Max,
    SpeedUp,
    Security,
    AdSource,
    Check,
    Restore,
}

impl Action {
    pub fn parse(s: &str) -> Option<Action> {
        match s.trim().to_ascii_lowercase().as_str() {
            "optimize" => Some(Action::Optimize),
            "max" | "maximum" => Some(Action::Max),
            "speedup" | "speed-up" => Some(Action::SpeedUp),
            "security" => Some(Action::Security),
            "adsource" | "ads" | "adfinder" => Some(Action::AdSource),
            "check" => Some(Action::Check),
            "restore" => Some(Action::Restore),
            _ => None,
        }
    }

    pub fn tag(self) -> &'static str {
        match self {
            Action::Optimize => "optimize",
            Action::Max => "max",
            Action::SpeedUp => "speedup",
            Action::Security => "security",
            Action::AdSource => "adsource",
            Action::Check => "check",
            Action::Restore => "restore",
        }
    }
}
