---
name: android-optimizer
description: Clean up and speed up any Android phone over ADB without root - remove preinstalled ads, junk and telemetry, block ads and trackers system-wide with Private DNS, turn off ad personalization, bigger text and display size, full app compilation, battery/storage/overheating/stuck-key diagnostics, a health score with before/after chart, malware check, pop-up ad source finder, and undo. Use when someone asks to clean, debloat, optimize or speed up an Android phone, remove ads, find what shows pop-up ads, fix "my phone lags / heats up / freezes / boot-loops into recovery", make text bigger for someone with poor eyesight, check a phone for viruses, connect a phone over Wi-Fi ADB, or restore removed apps.
---

# Android Optimizer

The tool is a single Rust binary, `android-optimizer` (source in `src/`). People run `start.bat` /
`start.sh` and get an interactive wizard. You cannot answer its prompts, so you **ask the person the same
questions yourself** (AskUserQuestion) and run it with `--yes` plus flags:

```
android-optimizer <action> --yes [flags]                 # a release build next to start.bat / start.sh
cargo run --release -- <action> --yes [flags]            # from source
```

| Flag | Meaning |
|---|---|
| `optimize` | safe lists + automatic optimizations (default with `--yes`) |
| `max` | also the `[max]` lists |
| `speedup` | cache + TRIM + full compilation only. Use after a system update |
| `security` | diagnostics + malware check, changes nothing |
| `adsource` | the app behind pop-up ads (run while the ad is on screen), changes nothing |
| `check` | dry run: diagnostics, health score, security check, the plan |
| `restore` | bring back removed apps and original settings (with `--yes`: everything) |
| `--text-size 0-3` | font x1.1/1.15/1.15 + display size x1.05/1.1/1.2. Or exact `--font-scale` / `--display-scale` |
| `--exclude a,b` | never remove these. Use it for "I use this app" |
| `--include x,y` | remove additionally (protected packages are still skipped) |
| `--background a,b` | no background activity (Xiaomi: set through its own battery screen) |
| `--mute a,b` | revoke notifications + `user-fixed` so the app cannot ask again (Android 13+) |
| `--no-dns` / `--no-compile` / `--reboot` | skip DNS / skip compilation / restart at the end |
| `--serial id`, `--connect ip:port`, `--pair ip:port --code N` | choose the phone / Wi-Fi debugging |

Root needs no flag: the tool detects existing root by itself (read-only) and stays silent without it.

## Workflow

1. **Connect.** `adb devices -l`.
   - Empty: the tool itself recognises the brand on the USB bus and prints the brand-exact steps; you can
     relay them. For a cable, wait in the background with `timeout 3600 adb wait-for-device`
     (run_in_background) instead of polling. If the computer shows no new USB device at all, the cable is
     charge-only or the port is damaged: suggest another cable or Wi-Fi.
   - Wi-Fi: the person opens Developer options → Wireless debugging → "Pair device with pairing code" and
     runs `adb pair <ip:port>` **themselves**, typing the code. Do not ask for the code in chat. Then run
     `adb connect <ip:port from the main Wireless debugging screen>` (or find it in `adb mdns services`).
   - `unauthorized`: they must tap Allow on the phone. `recovery`: see "Boot loops".
2. **Dry run:** `check --yes`. Read the device, warnings (storage ≥85%, battery, heat, stuck key, RAM), the
   health score, the security result, the removal list, and `skipped.txt` in the report folder.
3. **Ask the person** (one AskUserQuestion with up to four questions). Ask **only preferences**:
   - which listed apps they actually use (weather, music, video player, browser with bookmarks...);
   - whether heavy apps (TikTok, shops, streaming) should be limited in the background;
   - whether new apps since the last run should be limited or silenced;
   - text and display size (keep / large / larger / largest).
   Do **not** ask about safe automatic steps (lists' `[safe]` section, DNS, animations, cache, compile).
   Never remove banking, government, payment, messenger or app-store apps on your own.
   Malware: the security check flags `lists/malware.txt` (documented preinstalled riskware, e.g. Adups FOTA)
   AND catches the rest heuristically (hidden apps, sideloaded APKs, device admins, Accessibility, proxy) -
   most preinstalled malware uses random package names, so the heuristics matter more than the list.
4. **Unknown brand or few matches:** look at `adb shell pm list packages -s` and `pm list packages -3 -i`.
   Add obvious ads, telemetry and partner preinstalls (empty installer or the OEM store) to `lists/<brand>.txt`
   and the brand to `src/phone/brands.rs`, or pass `--include` once. When unsure about a package, leave it.
5. **Run** `optimize|max --yes ...` in the background (run_in_background). Full compilation of ~50 apps takes
   up to an hour on budget chips; the output shows a progress bar per app and `[k/5]` per stage.
6. **Verify.** Report folder `reports/<date>_<brand>_<model>_<action>/`: `device.txt`, `removed.tsv`,
   `tweaks.tsv`, `background.tsv`, `notifications.tsv`, `security.txt`, `skipped.txt`,
   `packages-before.txt` / `packages-after.txt`. The end of the output has the before/after chart (health
   score, app start time, free RAM and storage). Spot-check on the phone: `ping -c1 doubleclick.net` →
   0.0.0.0/127.0.0.1 while `ping google.com` → a real IP; `settings get system font_scale`; `wm density`;
   compile states (`dumpsys package dexopt | grep -oE 'status=[a-z-]+' | sort | uniq -c`).
   Screenshot (`adb exec-out screencap -p > shot.png`) a few apps to confirm big text still fits.
7. **Reboot** only with the person's consent and after checking that no volume key is stuck.
8. **Undo:** `restore --yes`, or one app: `adb shell pm install-existing --user 0 <pkg>`
   (`pm enable --user 0 <pkg>` if it was disabled).

## Preparation steps (the tool prints them per brand)

1. Settings → About phone → tap "Build number" 7 times (Xiaomi: "OS version"/"MIUI version";
   Samsung: Software information → Build number; vivo: "Software version").
2. Developer options → "USB debugging". Xiaomi: also "USB debugging (Security settings)" - needs a SIM card,
   a Xiaomi account and mobile data; if it turns itself off, enable "Install via USB" first and retry.
   Without it, removal works but `settings put`, `wm`, `input` fail with SecurityException
   (check: `getprop persist.security.adbinput` = 1).
3. Connect and tap "Allow USB debugging?" → "Always allow".

Best order after a factory reset: sign in to Google → let Play restore apps → optimize. Play Auto Install and
backups can bring removed apps back; run again if they did. Apps restored from the person's own backup
(banking, social, shops) are theirs - do not remove them without asking.

## Safety rules

- Only reversible operations: `pm uninstall -k --user 0`, `pm disable-user`, `settings put`, `wm density`,
  appops, permission flags. No writes to system partitions.
- **Never gain root and never ship/build a root exploit or "one-click root" file.** That is a hard line,
  whatever the framing. Root = the owner unlocking the bootloader (wipes data) and installing Magisk
  themselves. The tool only *detects* existing root (`src/phone/root.rs`, read-only: `command -v su`, then
  `su -c id`) and then reads root-only files (cpufreq caps, GPU load, battery sysfs). Without root nothing
  root-related runs or prints. If asked for a root file, refuse and point to the README Root section; do not
  suggest hosting or alternative ways to obtain an exploit.
- Bootloader unlock or flashing only on an explicit request, with the risks spelled out and a separate
  confirmation - and even then you guide, you don't ship an exploit.
- Never weaken `lists/protected.txt` to get a package through.
- Do not touch user data (photos, files, chats). Caches only via `pm trim-caches`.
- Never accept consent or terms dialogs for the person (e.g. Play's "app install optimization"); close them
  with `KEYCODE_BACK`. Never type passwords, PINs or pairing codes.
- Private DNS is strict mode: the tool verifies resolution and rolls back. "No internet" after a run →
  `adb shell settings put global private_dns_mode opportunistic`.

## Lessons from real devices

- **Compilation:** after a factory reset ~60% of packages are `status=verify` (not compiled). `compile -a -m speed`
  takes hours on budget chips (one Google app alone 15+ min, plus 32-bit copies), so the tool compiles only
  apps with icons + System UI + the home screen (`src/tune/compile-apps.sh`, embedded in the binary). It needs
  ≥ 8 GB and ≥ 25% free: compiled code took ~6.5 GB on a 256 GB phone. A system update resets compilation
  (`pm.dexopt.boot-after-ota=verify`); the tool compares the build fingerprint with the last report.
  Android itself compiles at install (`pm.dexopt.install=speed-profile`) and overnight while charging;
  making that `speed` needs root (`setprop` is denied).
- **Xiaomi background limits:** PowerKeeper resets `RUN_ANY_IN_BACKGROUND` to `allow` on boot and its provider
  is closed to shell. The tool uses App info → Battery → "Close apps after 10 minutes...". Never pick
  "Soft restriction": despite its description it stops ALL background work (music stops). Media apps
  (`@media`) keep "Battery saver (recommended)".
- **Screen automation:** tap by text via `uiautomator dump` + XML (`src/phone/screen.rs`, flows in `src/tune/screen_flows.rs`), not by guessed
  coordinates. Read a dialog before pressing anything; `KEYCODE_HOME` cancels it. Launcher folders cannot be
  created with `input draganddrop` on MIUI (icons just move); ask the person instead. "Cleaner" on a Xiaomi
  home screen is a Security shortcut (`id=cleaner`): long-press → Remove.
- **Text size:** Android has no per-app font size. A large `font_scale` alone pushes buttons off screen in many
  apps; display size (`wm density`, never below 320 dp width) scales layouts proportionally. Prefer
  font ≤ 1.15 + display x1.1-1.2.
- **"Is it throttled on purpose?":** without root you can't decompile the firmware. Check instead: cpufreq
  `cpuinfo_max_freq` vs `scaling_cur_freq` under load, `dumpsys thermalservice` (Thermal Status), `low_power`,
  limiter packages (`com.xiaomi.joyose`, `com.samsung.android.game.gos`), `always_finish_activities`,
  `max_cached_processes`, `dumpsys power` wake locks, `top`.
- **Ads that remain:** YouTube/Instagram/TikTok serve ads from their content servers - DNS cannot block them
  without breaking the app. Privacy Sandbox: `am start -a android.adservices.ui.SETTINGS`.
  Google ad personalization: `am start -a com.google.android.gms.settings.ADS_PRIVACY` (the tool flips the
  switch on screens that have one; deleting the advertising ID is the person's decision).
- **Battery without root:** sysfs is often closed (Xiaomi); wear comes from `dumpsys batterystats`
  (`Estimated battery capacity` = design, `Max learned battery capacity` = real).
- **Calls "robotic" / not heard:** VoLTE (`settings get global volte_subscription0`), LTE signal
  (`dumpsys telephony.registry`), then a voice-recorder test: bad there too → microphone; fine → network.
- **Telemetry:** MQS, analytics, MiSight and MTK Logger are fine to remove. Keep `com.xiaomi.mtb`,
  `com.xiaomi.simactivate.service`, `com.miui.audiomonitor` (modem, SIM, audio) and Google mainline modules
  (`adservices`, `mainline.telemetry`). Never turn location off.
- **Changing the code:** `Adb::sh` passes the whole command as one argument, so the phone's own shell handles
  pipes and redirects. Parse adb output once, in a pure function next to where it comes in, and cover it with
  a test in that layer's `tests.rs` (layers are described in `src/main.rs`). Run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.

## Boot loops and recovery

- **Check the volume keys first.** The diagnostics flag a stuck key: dozens of
  `adjustSuggestedStreamVolume ... from android/android` every ~50 ms in `dumpsys audio`. A held Volume Up
  sends Xiaomi and many other phones into recovery on every boot - it looks like a boot loop, and Wipe Data does
  not fix it.
- Xiaomi "Reboot / Wipe Data / Connect with MIAssistant" is Mi Recovery; Wipe Data erases everything.
- If it keeps happening with free keys: storage (eMMC/UFS) or corrupted firmware. Look at
  `adb logcat -b crash -d` and `dumpsys diskstats`; the fix is the official firmware via the manufacturer's tool
  or an authorized service. Do not suggest unofficial flashers or firmware sites.

## Custom ROMs

Don't suggest them by default. Unlocking wipes data, is very restricted on Xiaomi HyperOS, impossible on Huawei,
permanently trips Knox on Samsung, and banking/government apps may stop working. Discuss only if asked and the
phone no longer gets updates; list the risks honestly.
