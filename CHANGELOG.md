# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
