//! droidbridge-rs: auto-connect ADB over Wi-Fi when the phone comes back.

mod adb;
mod config;
mod scanner;
mod tray;

use anyhow::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("droidbridge_rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let cfg = config::Config::load_or_create()?;
    println!(
        "droidbridge_rs {} (config: {})",
        env!("CARGO_PKG_VERSION"),
        config::Config::path()?.display()
    );
    let _ = cfg; // used from Stage 3 on (mode dispatch)
    Ok(())
}
