//! Bluetooth connect-event guard: Kernel-PnP event 410 ("device started")
//! within a recent window, matched against the phone's BT MAC.
//!
//! Reads the event log via `wevtutil.exe` (same subprocess pattern as the
//! adb layer) instead of pulling in the windows-rs crate: the XPath filter
//! does both the EventID and the time-window cut server-side.

use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

const LOG_NAME: &str = "Microsoft-Windows-Kernel-PnP/Configuration";

/// XPath selecting event 410 records created within `window` of now.
fn xpath(window: Duration) -> String {
    format!(
        "*[System[(EventID=410) and TimeCreated[timediff(@SystemTime) <= {}]]]",
        window.as_millis()
    )
}

#[cfg(test)]
mod xpath_shape {
    use super::*;

    /// Byte-exact check: a stray paren here once made wevtutil reject the
    /// whole query with "syntax error at position 71" while the guard
    /// silently reported "no BT events".
    #[test]
    fn xpath_is_exactly_balanced() {
        let q = xpath(Duration::from_secs(90));
        assert_eq!(
            q,
            "*[System[(EventID=410) and TimeCreated[timediff(@SystemTime) <= 90000]]]"
        );
        assert_eq!(q.matches('(').count(), q.matches(')').count());
        assert_eq!(q.matches('[').count(), q.matches(']').count());
    }
}

/// True if the raw XML of recent 410 events matches `mac`.
/// Case-insensitive like the PS original's `-match`; an empty MAC means
/// "no filter configured - any BT device counts".
pub fn event_xml_matches_mac(xml: &str, mac: &str) -> bool {
    if mac.is_empty() {
        return true;
    }
    xml.to_lowercase().contains(&mac.to_lowercase())
}

/// Did OUR phone (or any BT device, when no MAC is configured) start
/// within the last `window`?
pub fn recent_bt_connect(device_bt_mac: &str, window: Duration) -> Result<bool> {
    let mut cmd = Command::new("wevtutil.exe");
    cmd.args([
        "qe",
        LOG_NAME,
        &format!("/q:{}", xpath(window)),
        "/f:xml",
        // generous count: a single BT connect spawns one 410 per profile,
        // and simultaneous devices (headphones) must not push OUR phone's
        // events out of the result window
        "/c:50",
        "/rd:true",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .context("running wevtutil.exe (event log query)")?;
    let xml = String::from_utf8_lossy(&out.stdout);
    if !xml.contains("<Event") {
        return Ok(false);
    }
    Ok(event_xml_matches_mac(&xml, device_bt_mac))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BT_EVENT: &str = "<Event><System><EventID>410</EventID></System>\
        <EventData><Data Name='DeviceInstanceId'>\
        BTHENUM\\DEV_FEDCBA987654\\7&2c1f&0&AABBCCDD_C00000000\
        </Data></EventData></Event>";

    #[test]
    fn xpath_pins_event_id_and_window() {
        let q = xpath(Duration::from_secs(90));
        assert!(q.contains("EventID=410"));
        assert!(q.contains("timediff(@SystemTime) <= 90000"));
    }

    #[test]
    fn matches_mac_case_insensitive() {
        assert!(event_xml_matches_mac(BT_EVENT, "FEDCBA987654"));
        assert!(event_xml_matches_mac(BT_EVENT, "fedcba987654"));
        assert!(!event_xml_matches_mac(BT_EVENT, "000000000000"));
    }

    #[test]
    fn empty_mac_means_any_device() {
        assert!(event_xml_matches_mac(BT_EVENT, ""));
        assert!(event_xml_matches_mac("anything at all", ""));
    }

    #[test]
    fn non_bt_event_does_not_match() {
        let usb = "<Event><EventData><Data Name='DeviceInstanceId'>\
            USB\\VID_0BDA&amp;PID_8179\\00E04C0001\
            </Data></EventData></Event>";
        assert!(!event_xml_matches_mac(usb, "FEDCBA987654"));
    }
}

#[cfg(test)]
mod live_debug {
    use super::*;

    /// Prints the raw window content and the guard verdict — for
    /// investigating why a live phone connect was rejected.
    #[test]
    #[ignore = "live stand: run right after a phone BT toggle"]
    fn live_debug_raw_window() {
        let cfg = crate::config::Config::load_or_create().unwrap();
        eprintln!("deviceBtMac = {:?}", cfg.device_bt_mac);

        let mut cmd = Command::new("wevtutil.exe");
        cmd.args([
            "qe",
            LOG_NAME,
            &format!("/q:{}", xpath(Duration::from_secs(300))),
            "/f:xml",
            "/c:50",
            "/rd:true",
        ]);
        let out = cmd.output().unwrap();
        let raw = String::from_utf8_lossy(&out.stdout);
        eprintln!("--- raw window ({} bytes) ---", raw.len());
        let q = format!("/q:{}", xpath(Duration::from_secs(300)));
        eprintln!("xpath string: {:?}", q);
        eprintln!("cmd debug: {:?}", cmd);
        eprintln!("{}", &raw[..raw.len().min(3000)]);
        eprintln!(
            "contains <Event: {}, verdict: {}",
            raw.contains("<Event"),
            event_xml_matches_mac(&raw, &cfg.device_bt_mac)
        );
        eprintln!(
            "recent_bt_connect(90s): {:?}",
            recent_bt_connect(&cfg.device_bt_mac, Duration::from_secs(90))
        );
    }
}
