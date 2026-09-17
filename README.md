# droidbridge-rs

Native Windows tray utility that auto-connects ADB to your Android phone over
Wi-Fi the moment the phone reconnects via Bluetooth — plus one-click
[scrcpy](https://github.com/Genymobile/scrcpy) mirroring.

A Rust port of [DroidBridge](https://github.com/genxp/DroidBridge): one static
`droidbridge.exe`, ~5 ms cold start, a few MB of RAM, no PowerShell, no
ExecutionPolicy dance, no runtime dependencies.

![status](https://img.shields.io/badge/status-work--in--progress-orange)
![platform](https://img.shields.io/badge/platform-Windows%2010%2F11-blue)
![license](https://img.shields.io/badge/license-MIT-green)
![msrv](https://img.shields.io/badge/MSRV-1.85%2B-9c6f3f)

> **Status:** the Rust rewrite is in active development. The feature set is
> being ported stage by stage from the battle-tested PowerShell original; the
> CLI modes land first, tray and GUI follow. The table in
> [What droidbridge-rs does](#what-droidbridge-rs-does) tracks parity.

## The problem

On Android 11+ the "Wireless debugging" port is **ephemeral** — it changes
every time you toggle the switch, so `adb connect` is never just a one-liner.
And nothing reconnects for you when the phone comes back into range.

## What droidbridge-rs does

- **Bluetooth-triggered**: a Windows Scheduled Task listens for the Kernel-PnP
  event that fires when your paired phone starts its Bluetooth profiles. When
  it does, droidbridge connects ADB over Wi-Fi automatically. No polling, no
  background daemons — the connect runs only at the moment of the Bluetooth
  event.
- **Port discovery, three tiers**: cached last known port → mDNS
  (`_adb-tls-connect._tcp`) → full async TCP scan of the Android ephemeral
  range (32768–60999).
- **Works over any IP the phone answers on**: home LAN, phone hotspot, or a
  tailnet/overlay address — a Tailscale IP is the most convenient because it
  works at home and away.
- **Tray app**: connect / mirror / pair / settings from the notification area.
- **Native**: single exe, instant start, single-digit memory footprint.

| Capability | Status |
|---|---|
| Config (PS-compatible `config.json`) | ✅ |
| Port cache + mDNS + range scan | ✅ |
| Headless `--connect` / `--mirror` / `--bt-check` | ✅ |
| Rotating log (`droidbridge.log`) | ✅ |
| Tray icon + menu (single instance, double-click = Mirror) | ✅ |
| Settings / Pair dialogs | ✅ |

## How it works

```
Bluetooth connect (paired phone)
        │  Kernel-PnP event 410 ("device started")
        ▼
Scheduled Task  ──►  droidbridge.exe --connect --bt-check
                        │  1. confirm the event matches YOUR phone's BT MAC
                        │  2. wait (up to 5 min) for Wi-Fi / overlay network
                        │  3. adb connect <phone>:<port>
                        │     port = cache → mDNS → ephemeral-range scan
                        ▼
                     ADB over Wi-Fi ready (scrcpy optional)
```

## Requirements

- Windows 10 or 11
- Android 11+ with *Developer options → Wireless debugging* enabled
- adb.exe (bundled with scrcpy, or from Android SDK platform-tools)
- Optional: scrcpy for mirroring

## Install

### Prebuilt binary

Grab `droidbridge.exe` from [Releases](../../releases) and put it anywhere on
`PATH`.

> **SmartScreen:** the exe is unsigned, so Windows may show "Windows protected
> your PC". Click *More info → Run anyway*. Or build from source — see below.

### Build from source

```
git clone <this repo>
cd droidbridge-rs
cargo build --release
# → target\release\droidbridge_rs.exe
```

## First-run setup (one time)

1. Phone: *Settings → Developer options → Wireless debugging* → ON.
2. Open *Pair device with pairing code* on the phone.
3. Run `droidbridge_rs.exe --pair` and enter the IP, pairing port and the
   6-digit code from the phone screen.
4. Edit `%APPDATA%\DroidBridge\config.json`: set the phone IP (recommended:
   the phone's Tailscale IP, it works both at home and away) and optionally
   its Bluetooth MAC (found in the phone's Bluetooth details), so only *your*
   phone wakes the bridge — not your headphones.

Done. From now on: phone connects via Bluetooth → ADB is connected within
seconds (the tool waits up to 5 minutes for Wi-Fi to come up).

## CLI reference

```
droidbridge_rs.exe --connect        connect to the configured phone, then exit
droidbridge_rs.exe --mirror         connect, then launch scrcpy
droidbridge_rs.exe --bt-check       (with --connect) only act on a fresh
                                     Bluetooth connect event of YOUR phone
droidbridge_rs.exe --pair           pairing dialog
droidbridge_rs.exe --settings       settings dialog
droidbridge_rs.exe                  tray mode (menu: Connect now / Mirror /
                                     Pair device... / Settings... / Open log / Exit)
```

Exit codes: `0` success (or already connected), non-zero on failure. The
headless modes are quiet and fast — designed to be called from a Scheduled
Task.

## Config reference (`%APPDATA%\DroidBridge\config.json`)

Config is **shared with the PowerShell original** — if you already run
DroidBridge, droidbridge-rs picks up the same file.

| Key | Meaning |
|---|---|
| `adbPath` / `scrcpyPath` | full paths; empty = autodetect (scrcpy-bundled adb preferred) |
| `deviceHost` | phone IP (Tailscale IP recommended) |
| `deviceBtMac` | phone BT MAC `AABBCCDDEEFF`; empty = any BT device triggers |
| `portMin` / `portMax` | ephemeral scan range (defaults 32768–60999) |
| `waitForWifiSeconds` | how long to wait for the network after a BT connect |
| `btTriggerEnabled` | whether the BT scheduled task is registered |
| `scrcpyOnConnect` | launch scrcpy automatically after every connect |

## Tailscale / VPN users

If you run a VPN with a default-route tunnel (e.g. a Tailscale exit node),
outbound connects to LAN addresses may fail instantly with `WSAEACCES (10013)`.
For Tailscale the fix is one command and keeps the exit node working:

```
tailscale set --exit-node-allow-lan-access=true
```

## FAQ

**Why a Rust port if the PowerShell version works?**
It does work — and stays available. The port buys distribution: one static
exe with zero runtime requirements, ~5 ms startup instead of a PowerShell
cold start, and a few MB of RAM in tray mode.

**Why Bluetooth and not a timer?**
A Bluetooth connect is a reliable "the user is at the desk" signal, and an
event trigger costs nothing — no process, no polling, no battery.

**Do I need to re-pair after phone reboot?**
No. Pairing keys are persistent. You only need to re-enable *Wireless
debugging* after a reboot (Android turns it off), and the port may change —
droidbridge will find it again.

**Is the port scan dangerous?**
It connects to TCP ports of a single host you configured (your own phone) in
the standard Linux ephemeral range. That is exactly what `adb mdns` would do,
just without the broken mDNS backend.

## License

MIT — see [LICENSE](LICENSE).
