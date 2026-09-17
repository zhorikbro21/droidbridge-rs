//! droidbridge-rs: auto-connect ADB over Wi-Fi when the phone comes back.

mod adb;
mod config;
mod portcache;
mod scanner;
mod tray;

use std::sync::LazyLock;

use anyhow::{Context, Result};

/// Shared tokio runtime for sync code paths that need async internals
/// (e.g. the port scanner called from the adb connect flow).
pub static RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
});

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("droidbridge_rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let cfg = config::Config::load_or_create()?;

    // Rudimentary dispatch; grows into full CLI in Stage 3.
    if args.iter().any(|a| a == "--connect") {
        let adb = adb::resolve_adb(&cfg).context("adb.exe not found - set adbPath in config")?;
        match adb::connect_phone(&adb, &cfg, true)? {
            Some(serial) => {
                println!("connected: {serial}");
                return Ok(());
            }
            None => anyhow::bail!("could not connect to {}", cfg.device_host),
        }
    }

    println!(
        "droidbridge_rs {} (config: {})",
        env!("CARGO_PKG_VERSION"),
        config::Config::path()?.display()
    );
    Ok(())
}
