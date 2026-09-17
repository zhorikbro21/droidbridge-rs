# Security Policy

## Supported versions

Only the latest release line receives security fixes.

## Reporting a vulnerability

Please open a **private security advisory** on GitHub
(*Security → Report a vulnerability*), or contact the maintainer directly.
Do not open a public issue for security problems.

## Scope notes

- droidbridge-rs runs entirely on your machine: no telemetry, no network
  calls except to the phone you configure and local adb.
- The config (`%APPDATA%\DroidBridge\config.json`) contains local network
  addresses and an optional device MAC — consider this before pasting it
  into bug reports.
