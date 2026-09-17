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
        "*[System[(EventID=410) and TimeCreated[timediff(@SystemTime) <= {})]]]",
        window.as_millis()
    )
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
        "/c:5",
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
