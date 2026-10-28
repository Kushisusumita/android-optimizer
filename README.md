# Android Optimizer

[![CI](https://github.com/Kushisusumita/android-optimizer/actions/workflows/ci.yml/badge.svg)](https://github.com/Kushisusumita/android-optimizer/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/Kushisusumita/android-optimizer?include_prereleases&sort=semver)](https://github.com/Kushisusumita/android-optimizer/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/made%20with-Rust-orange?logo=rust)](https://www.rust-lang.org)
[![Platforms](https://img.shields.io/badge/runs%20on-Windows%20%7C%20macOS%20%7C%20Linux-informational)](#quick-start)
[![Android](https://img.shields.io/badge/Android-any%20brand-3DDC84?logo=android&logoColor=white)](#what-it-does)
[![No root](https://img.shields.io/badge/root-not%20needed-success)](#root)

**One command to clean up any Android phone.** It removes preinstalled ads, junk and telemetry, blocks
ads and trackers across the whole phone, speeds it up, checks it for malware — and **everything can be
undone**. No root, no flashing, no warranty loss.

Made for people who can't afford a new phone every two years.

> It can't remove ads **inside** YouTube, Instagram or TikTok — those come from the same servers as the
> videos, so blocking them would break the app. Only their paid subscriptions remove those.

## Quick start

1. **Get it:** download the ZIP for your computer from
   [Releases](https://github.com/Kushisusumita/android-optimizer/releases) and unpack it.
   (From source: `git clone https://github.com/Kushisusumita/android-optimizer.git` — the first start builds
   it, which needs [Rust](https://rustup.rs).)
2. **Run:**
   - **Windows:** double-click **`start.bat`**
   - **Linux / macOS:** `./start.sh`
3. **Connect the phone** by USB. The tool recognises the brand as soon as the cable is in and shows the
   exact steps for that phone (where "Build number" is, how to turn on USB debugging, Xiaomi's extra
   "Security settings" switch). Tap **Allow** on the phone.

It asks a few personal questions up front, then runs on its own. If ADB isn't installed, it offers to
download Google's official version.

## What it does

Runs **automatically** (safe for everyone):

- Removes preinstalled ads, junk and telemetry from per-brand lists in [`lists/`](lists) — e.g. Xiaomi MSA
  ads and analytics, Facebook/Netflix/game preinstalls, silent installers. Apps **you** installed from a
  store are never touched.
- Blocks ads and trackers in every app and browser with Private DNS (verified; rolled back if the server is
  unreachable, so your internet never breaks).
- Turns off ad personalization, speeds up animations, stops constant Wi-Fi/Bluetooth scanning.
- Clears caches, runs storage TRIM and **fully compiles every app with an icon** — the biggest speed boost
  after a factory reset.
- **Security check** for viruses and spyware (see [Is it safe?](#is-it-safe)).

Shows you **how the phone is doing**, before and after:

- Hardware: chip, CPU cores and clocks, GPU, RAM; current CPU load and the busiest processes.
- A **health score** from 0 to 100 (storage, memory, battery wear, heat, a stuck key, leftover junk).
- A real speed measurement: how long a standard app takes to open from cold.
- Progress through each stage (`[2/5] … 40% done`), the score so far, and a **before/after chart** at the end.

Only **asks** about personal choices: which apps to keep, which heavy apps to limit in the background,
which apps to silence, and the text/display size.

## Actions

Without an action the tool shows a menu.

| Action | What happens |
|---|---|
| `optimize` (recommended) | Everything automatic above, plus the preference questions |
| `max` | Also removes optional apps (you choose) |
| `speedup` | Cache + compilation, no removals. **Run after a system update** |
| `security` | Diagnostics and the malware check. Changes nothing |
| `adsource` | Finds the app behind pop-up ads. Changes nothing |
| `check` | Shows what it would do. Changes nothing |
| `restore` | Brings back removed apps (all or chosen) and the original settings |

## Connecting

**USB.** Plug the phone in and tap **Allow** in "Allow USB debugging?" (tick "Always allow"). If the
computer does not see the phone at all, try another cable — many cables only charge.

**Wi-Fi** (Android 11+, phone and computer on the same network):

1. On the phone: Developer options → **Wireless debugging** → on → **Pair device with pairing code**.
2. Start with the address and code shown there: `start.bat --pair 192.168.1.23:41234 --code 123456`.

An already paired phone: `start.bat --connect 192.168.1.23:37011`.

## Is it safe?

- Apps are removed for the current user only (`pm uninstall --user 0`). The firmware is **not** modified,
  so the phone can't be bricked and a factory reset brings everything back.
- [`lists/protected.txt`](lists/protected.txt) is never touched: system, calls, SMS, camera, Play Store,
  Google services, keyboards, home screens, the security app, banking and government apps.
- Every change is recorded in `reports/`, and **Restore** undoes it.
- Decisions are remembered per phone in `keep/`: an app you keep is never offered for removal again.

**Malware detection.** Most preinstalled malware uses random, ever-changing package names, so the main
defense is **heuristic** — the security check flags hidden apps (no icon), apps installed from outside a
store (sideloaded APKs), device administrators, Accessibility abuse (how banking trojans read the screen),
apps allowed to install other apps, and traffic sent through a proxy. On top of that there is a short list
of **documented** malware/riskware families in [`lists/malware.txt`](lists/malware.txt) — e.g. **Adups
FOTA**, a backdoor that silently installs other malware. No fabricated package names are in that list;
contributions must cite a source.

`reports/` and `keep/` hold information about your phone and are kept out of git.

## Root

**The tool will never try to root your phone and ships no root exploit or "one-click root" file.** Getting
root means unlocking the bootloader yourself (which **wipes the phone** and can break banking/government
apps) and installing [Magisk](https://github.com/topjohnwu/Magisk). That is your decision and your
responsibility.

If your phone is **already rooted**, the tool notices it by itself and adds a few **read-only** checks:
whether the CPU is held below its hardware maximum frequency (deliberate throttling), the GPU load, and the
real battery wear. It only reads; it never writes and never gains root. On a phone without root you see
nothing about root at all.

## FAQ

**Will a system update undo the optimization?** Install updates — they matter for security. Settings (DNS,
text size, animations) survive, but **app compilation is reset** and a big update can bring removed apps
back. Run `speedup` (or `optimize`) afterwards; the tool notices the update and reminds you. Before
updating, make sure the battery is above 50% and no volume key is stuck.

**Will new apps be optimized?** Mostly yes, by themselves: ad blocking, text size and animations apply to
every app, and Android compiles new apps at install time and overnight while charging. Every run also lists
the apps installed since last time.

**Constant pop-up ads.** Run `adsource` **while an ad is on screen**: it names the app showing it, lists
apps allowed to draw over others, apps with powerful permissions and the most recently installed apps.

**Will all ads disappear?** System ads, browser ads and most in-app banners, yes. Ads inside YouTube,
Instagram and TikTok come from the same servers as the content, so blocking them breaks the app — only
their paid subscriptions remove those. "Modded, ad-free" versions of apps are a common source of malware.

**Text is too small, but a huge font breaks apps.** Android has no per-app font size. That is why the size
presets mostly enlarge the **display size**: text, buttons and icons grow together and layouts still fit,
while the font itself is raised only moderately.

**Is the phone being slowed down on purpose?** Without root nobody can read the firmware's frequency
limits; the tool shows what is visible — thermal throttling, CPU load, battery wear, full storage. On an
already-rooted phone it also compares the allowed and the hardware CPU maximum. Real slowdowns usually come
from preinstalled junk in the background, full storage, a worn battery and heavier apps — the tool handles
the first and tells you about the rest.

**The phone keeps booting into the "Reboot / Wipe data" menu.** Check the volume keys first: a stuck
**Volume Up** sends many phones into recovery on every start, and a factory reset does not fix it. The
diagnostics detect a stuck key. Remove the case, press the key a few times, clean the gap; otherwise a
repair shop replaces it cheaply.

**People can't hear me on calls / my voice sounds robotic.** Record yourself with the voice recorder. If
it sounds bad there too, the microphone is blocked or damaged (the small hole by the charging port). If
the recording is fine, compare with a WhatsApp/Telegram call; if those are fine, the problem is the network
or VoLTE.

**"Security", "Themes", "Cleaner" icons on a Xiaomi home screen.** "Cleaner" is just a shortcut:
long-press → Remove. Security and Themes are part of the system — removing them breaks the phone; put them
into a folder.

**Should I install a custom ROM?** For most people, no. Unlocking the bootloader wipes the phone, is very
restricted on Xiaomi HyperOS and impossible on Huawei, permanently trips Knox/Samsung Pay, and
banking/government apps may stop working.

## Why phones slow down over time

Software causes, which this tool fixes: preinstalled junk and ads running in the background, heavy
manufacturer skins, apps that get heavier with every update.

Hardware causes, which cleaning cannot fix:

- **Full storage.** Above ~85% used, flash storage slows down sharply. Keep 15-20% free.
- **A worn battery** can't deliver peak current, so the phone throttles, heats up and shuts down.
  A replacement costs a fraction of a new phone.
- **Heat.** Don't charge and play at the same time, and take off a thick case while charging.

## Advanced

```
start.bat [action] [options]        # Windows   (or android-optimizer.exe directly)
./start.sh [action] [options]       # Linux / macOS
```

| Option | Meaning |
|---|---|
| `-y`, `--yes` | no questions: safe defaults only |
| `--text-size 0-3` or `--font-scale 1.15 --display-scale 1.2` | bigger text and display |
| `--exclude a,b` / `--include x,y` | never remove (remembered in `keep/`) / additionally remove packages |
| `--background a,b` | stop apps from running in the background |
| `--mute a,b` | turn notifications off and stop the apps from asking again (Android 13+) |
| `--no-dns`, `--no-compile`, `--reboot` | skip ad-blocking DNS / skip compilation / restart at the end |
| `--serial id`, `--connect ip:port`, `--pair ip:port --code 123456` | choose the phone / Wi-Fi debugging |
| `--dns host` | another Private DNS filter (default `dns.adguard-dns.com`) |
| `--lists folder`, `--adb path` | use other lists / a specific adb |

`android-optimizer --help` prints the same list.

## Disclaimer

This software is provided "as is", without warranty of any kind (see [LICENSE](LICENSE)). You use it at
your own risk and you are responsible for what you remove. Removing or disabling system apps can change how
a phone behaves; always start with `check` and keep the report so you can `restore`. Ad blocking uses a
public DNS resolver of your choice. This project is not affiliated with, endorsed by, or connected to
Google or any device manufacturer; all product names and brands are the property of their owners.

## Contributing

Pull requests are very welcome, especially coverage for more phone brands.

- **Lists** live in [`lists/`](lists), one plain-text file per brand family. Format:
  `package.name  # description  [@needs replacement] [@partner] [@media]`.
  `[safe]` is removed by `optimize`, `[max]` by `max`. A new brand also goes into the brand registry in
  [`src/brands.rs`](src/brands.rs) (detection, USB vendor ID, setup steps).
- Find package names with `adb shell pm list packages`. Good reference sources:
  [Universal Android Debloater NG](https://github.com/Universal-Debloater-Alliance/universal-android-debloater-next-generation)
  and its [package list](https://github.com/Universal-Debloater-Alliance/universal-android-preinstalled-lists).
- `lists/malware.txt` is only for **documented** preinstalled malware/riskware — cite a source in the PR.
- **When in doubt, leave a package out.** Leaving junk behind is better than breaking someone's phone.
- **Code** is plain Rust with no dependencies, one responsibility per module (see [`src/main.rs`](src/main.rs)),
  written the [ub-mode](https://github.com/unclebob/ub-mode) way: a failing test first, small functions,
  one decision in one place, no dead code, no silenced warnings. Before a PR:

  ```bash
  cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
  ```

  and run `start.bat check` on a real phone before and after your change.

## AI assistant

Step-by-step instructions for an AI coding assistant live in [`skill/SKILL.md`](skill/SKILL.md), so you
can let an assistant do the whole thing and explain the results. It asks you the same questions the wizard
does and never needs root.

- **Claude Code:** copy the `skill/` folder to `.claude/skills/android-optimizer/`, then say
  "optimize my Android phone".
- **Codex, Cursor, Gemini, Grok, Copilot or any other assistant:** open this repo and tell it to follow
  `skill/SKILL.md` — for example *"Optimize the connected Android phone following skill/SKILL.md"*.

## License

[MIT](LICENSE).
