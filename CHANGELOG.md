# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] - 2026-09-17

### Added

- Windows installer (Inno Setup): installs to Program Files with bundled
  adb + scrcpy, registers the Bluetooth auto-connect task, optional tray
  autostart and desktop shortcut; proper uninstaller.

## [Unreleased]

### Added

- Tray mirror watcher: when *Mirror automatically on every connect* is
  enabled, the mirror opens by itself whenever the phone appears on adb
  (covers adb's own mDNS auto-connect path). Max 2 tries per appearance;
  a manually closed mirror is never re-opened until the phone returns.

## [0.1.1] - 2026-09-17

### Added

- Self-installation: `--install` / `--uninstall` register and remove the
  Bluetooth auto-connect scheduled task for the running exe (no admin
  needed); also available as buttons in the Settings dialog.
- Portable distribution: the release now ships a zip with
  `droidbridge_rs.exe + adb.exe + scrcpy.exe`; the exe looks for
  adb/scrcpy in its own folder first.
- First-run convenience: starting the tray with no phone configured
  opens the Settings dialog automatically.
- UI: clearer Mirror toggle label; ASCII arrows (bundled egui font has
  no "→" glyph).

## [0.1.0] - 2026-09-17

### Added

- Project scaffold: tokio, serde, serde_json, anyhow, mdns-sd dependencies.
- CI pipeline (fmt, clippy, test, release build) and release workflow.
- Repository infrastructure: README, CHANGELOG, CONTRIBUTING, SECURITY,
  issue/PR templates, Dependabot, EditorConfig, rust-toolchain.
- PS-compatible config module (autocreates `%APPDATA%\DroidBridge\config.json`).
- adb layer: adb.exe autodetect (scrcpy-sibling first), devices parser,
  connect flow with cache → mDNS → range-scan tiers, zombie-port rejection.
- Async ephemeral-range TCP scanner (chunked, per-connect timeout, retry).
- Port cache `portcache.json` (accepts both PS JSON shapes).
- Headless CLI: `--connect`, `--mirror` (single scrcpy instance),
  `--bt-check` (Kernel-PnP event 410 guard via wevtutil, MAC filter,
  network wait up to `waitForWifiSeconds`).
- Rotating log `droidbridge.log` shared with the PowerShell original.
- Tray: menu parity, double-click = Mirror, tooltip status updates,
  named-mutex single instance.
- Pair and Settings dialogs (egui), one dialog process each; the tray
  hot-reloads the config before every action.
- Size-optimized release profile (LTO, strip, opt-level=z): ~9 MB exe.
