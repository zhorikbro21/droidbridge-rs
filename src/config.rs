//! PS-compatible configuration: `%APPDATA%\DroidBridge\config.json`.
//!
//! Key names (camelCase) and defaults are shared with the PowerShell
//! original, so both tools read the same file. Unknown/missing fields fall
//! back to defaults instead of failing.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Full path to adb.exe; empty = autodetect.
    pub adb_path: String,
    /// Full path to scrcpy.exe; empty = autodetect.
    pub scrcpy_path: String,
    /// Phone IP (LAN / hotspot / tailnet).
    pub device_host: String,
    /// Phone Bluetooth MAC `AABBCCDDEEFF`; empty = any BT device triggers.
    pub device_bt_mac: String,
    /// Ephemeral scan range lower bound.
    pub port_min: u16,
    /// Ephemeral scan range upper bound.
    pub port_max: u16,
    /// How long to wait for Wi-Fi after a BT connect event.
    pub wait_for_wifi_seconds: u64,
    /// Whether the BT scheduled task is registered.
    pub bt_trigger_enabled: bool,
    /// Launch scrcpy automatically after every connect.
    pub scrcpy_on_connect: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adb_path: String::new(),
            scrcpy_path: String::new(),
            device_host: String::new(),
            device_bt_mac: String::new(),
            port_min: 32768,
            port_max: 60999,
            wait_for_wifi_seconds: 300,
            bt_trigger_enabled: true,
            scrcpy_on_connect: false,
        }
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let appdata = std::env::var("APPDATA").context("%APPDATA% is not set")?;
        Ok(PathBuf::from(appdata)
            .join("DroidBridge")
            .join("config.json"))
    }

    /// Load the config, creating it with defaults on first run.
    pub fn load_or_create() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            let cfg = Self::default();
            cfg.save().context("creating default config")?;
            return Ok(cfg);
        }
        Self::load()
    }

    fn load() -> Result<Self> {
        let path = Self::path()?;
        let raw =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config =
            serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_ps_compatible_keys() {
        let json = serde_json::to_value(Config::default()).unwrap();
        for key in [
            "adbPath",
            "scrcpyPath",
            "deviceHost",
            "deviceBtMac",
            "portMin",
            "portMax",
            "waitForWifiSeconds",
            "btTriggerEnabled",
            "scrcpyOnConnect",
        ] {
            assert!(json.get(key).is_some(), "missing PS key: {key}");
        }
        assert_eq!(json["portMin"], 32768);
        assert_eq!(json["portMax"], 60999);
        assert_eq!(json["waitForWifiSeconds"], 300);
        assert_eq!(json["btTriggerEnabled"], true);
        assert_eq!(json["scrcpyOnConnect"], false);
    }

    #[test]
    fn parses_ps_example_config() {
        let raw = r#"{
            "adbPath": "C:/bin/adb.exe",
            "scrcpyPath": "",
            "deviceHost": "192.168.1.50",
            "deviceBtMac": "AABBCCDDEEFF",
            "portMin": 32768,
            "portMax": 60999,
            "waitForWifiSeconds": 300,
            "btTriggerEnabled": true,
            "scrcpyOnConnect": false
        }"#;
        let cfg: Config = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.adb_path, "C:/bin/adb.exe");
        assert_eq!(cfg.device_host, "192.168.1.50");
        assert_eq!(cfg.device_bt_mac, "AABBCCDDEEFF");
        assert_eq!(cfg.port_min, 32768);
        assert_eq!(cfg.port_max, 60999);
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let cfg: Config = serde_json::from_str("{}").unwrap();
        assert_eq!(cfg.port_min, 32768);
        assert_eq!(cfg.port_max, 60999);
        assert_eq!(cfg.wait_for_wifi_seconds, 300);
        assert!(cfg.bt_trigger_enabled);
    }
}
