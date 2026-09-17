//! droidbridge-rs: auto-connect ADB over Wi-Fi when the phone comes back.

mod adb;
mod bt;
mod config;
mod gui;
mod log;
mod portcache;
mod scanner;
mod tray;

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;

use crate::config::Config;

/// Shared tokio runtime for sync code paths that need async internals
/// (e.g. the port scanner called from the adb connect flow).
pub static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

#[derive(Parser)]
#[command(
    name = "droidbridge_rs",
    version,
    about = "Auto-connect ADB to your Android phone over Wi-Fi"
)]
struct Cli {
    /// Connect to the configured phone, then exit
    #[arg(long)]
    connect: bool,
    /// Connect, then launch scrcpy
    #[arg(long)]
    mirror: bool,
    /// Only act if our phone just (re)connected via Bluetooth
    #[arg(long)]
    bt_check: bool,
    /// Open the pairing dialog (own process, launched by the tray)
    #[arg(long)]
    pair: bool,
    /// Open the settings dialog (own process, launched by the tray)
    #[arg(long)]
    settings: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load_or_create()?;

    if cli.pair {
        return gui::run_pair();
    }
    if cli.settings {
        return gui::run_settings();
    }

    let want_connect = cli.connect || cli.mirror || cli.bt_check;
    if !want_connect {
        if single_instance_running() {
            println!("droidbridge_rs is already running (tray)");
            return Ok(());
        }
        tray::run(cfg)?;
        return Ok(());
    }

    if cfg.device_host.is_empty() {
        anyhow::bail!("no phone IP configured - set deviceHost in config");
    }

    // BT guard: not our phone (or nothing at all) - stay quiet and cheap.
    if cli.bt_check {
        let fresh = bt::recent_bt_connect(&cfg.device_bt_mac, Duration::from_secs(90))?;
        log::write(if fresh {
            "bt-check: our phone (re)connected via Bluetooth"
        } else {
            "bt-check: no matching recent BT event, exiting"
        });
        if !fresh {
            return Ok(());
        }
    }
    // The phone just came into BT range; give its Wi-Fi time to come up.
    if cli.bt_check {
        wait_for_network(&cfg);
    }

    let adb = adb::resolve_adb(&cfg).context("adb.exe not found - set adbPath in config")?;
    match adb::connect_phone(&adb, &cfg, true)? {
        Some(serial) => {
            println!("connected: {serial}");
            log::write(&format!("connected: {serial}"));
            if cli.mirror || cfg.scrcpy_on_connect {
                adb::launch_scrcpy_once(&cfg)?;
                log::write("scrcpy launched");
            }
            Ok(())
        }
        None => {
            log::write(&format!("connect failed on {}", cfg.device_host));
            anyhow::bail!("could not connect to {}", cfg.device_host)
        }
    }
}

/// Poll until the phone is reachable, then proceed regardless (the
/// connect flow reports the failure if it is not). The config may name
/// the tailnet IP while adb actually answers on the LAN/hotspot path,
/// so mDNS-advertised endpoints are polled too.
fn wait_for_network(cfg: &Config) {
    let port = portcache::load().first().copied().unwrap_or(53);
    let deadline = Instant::now() + Duration::from_secs(cfg.wait_for_wifi_seconds);
    loop {
        if scanner::tcp_probe(&cfg.device_host, port, Duration::from_millis(1200)) {
            return;
        }
        for ep in adb::native_mdns_endpoints(Duration::from_secs(2)) {
            if scanner::tcp_probe(&ep.host, ep.port, Duration::from_millis(1200)) {
                log::write(&format!(
                    "network wait: deviceHost {} unreachable, adb lives on {}:{}",
                    cfg.device_host, ep.host, ep.port
                ));
                return;
            }
        }
        if Instant::now() >= deadline {
            log::write("network wait: deadline hit, proceeding anyway");
            return;
        }
        std::thread::sleep(Duration::from_secs(10));
    }
}

/// Named-mutex single instance guard for tray mode.
fn single_instance_running() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
        use windows_sys::Win32::System::Threading::CreateMutexW;
        let name: Vec<u16> = "Local\\DroidBridgeRsSingleton\0".encode_utf16().collect();
        unsafe {
            CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
            GetLastError() == ERROR_ALREADY_EXISTS
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}
