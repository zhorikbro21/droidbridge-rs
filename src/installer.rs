//! Self-installation: registers/removes the Bluetooth-trigger scheduled
//! task pointing at the running exe. The task launches a wscript wrapper
//! so nothing flashes on screen (same pattern as the PowerShell
//! reference's install.ps1). Needs no admin: InteractiveToken task.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

pub const TASK_NAME: &str = "DroidBridgeRsAutoConnect";

fn data_dir() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var("APPDATA").context("%APPDATA% is not set")?)
            .join("DroidBridge"),
    )
}

fn schtasks(args: &[&str]) -> Result<(bool, String)> {
    let mut cmd = Command::new("schtasks.exe");
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().context("running schtasks.exe")?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok((out.status.success(), text))
}

/// Register the BT-trigger task. Overwrites an existing task.
pub fn install() -> Result<String> {
    let exe = std::env::current_exe().context("locating current exe")?;
    let dir = data_dir()?;
    fs::create_dir_all(&dir)?;

    // hidden launcher: wscript keeps the console invisible
    let vbs = dir.join("run-hidden.vbs");
    let cmd = format!("\"{}\" --connect --bt-check", exe.display());
    let line = format!(
        "CreateObject(\"WScript.Shell\").Run \"{}\", 0, False",
        cmd.replace('"', "\"\"")
    );
    fs::write(&vbs, line).with_context(|| format!("writing {}", vbs.display()))?;

    let subscription = "<QueryList><Query Id=\"0\" Path=\"Microsoft-Windows-Kernel-PnP/Configuration\"><Select Path=\"Microsoft-Windows-Kernel-PnP/Configuration\">*[System[EventID=410]]</Select></Query></QueryList>";
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.4" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>droidbridge-rs: adb connect on Bluetooth connect</Description></RegistrationInfo>
  <Triggers><EventTrigger><Enabled>true</Enabled><Subscription>{sub}</Subscription></EventTrigger></Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT10M</ExecutionTimeLimit>
    <Enabled>true</Enabled>
    <StartWhenAvailable>true</StartWhenAvailable>
  </Settings>
  <Actions Context="Author"><Exec><Command>wscript.exe</Command><Arguments>"{vbs}"</Arguments></Exec></Actions>
</Task>"#,
        sub = xml_escape(subscription),
        vbs = vbs.display(),
    );

    let task_xml = dir.join("task.xml");
    // schtasks requires UTF-16 with BOM
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    fs::write(&task_xml, bytes)?;

    let (ok, text) = schtasks(&[
        "/Create",
        "/F",
        "/TN",
        TASK_NAME,
        "/XML",
        &task_xml.to_string_lossy(),
    ])?;
    if !ok {
        anyhow::bail!("schtasks: {}", text.trim());
    }
    let _ = fs::remove_file(&task_xml);
    crate::log::write("BT auto-connect task installed");
    Ok(format!("installed ({})", exe.display()))
}

/// Remove the task and the wrapper script.
pub fn uninstall() -> Result<String> {
    let (ok, text) = schtasks(&["/Delete", "/F", "/TN", TASK_NAME])?;
    if !ok && !text.contains("не существует") && !text.contains("does not exist") {
        anyhow::bail!("schtasks: {}", text.trim());
    }
    if let Ok(dir) = data_dir() {
        let _ = fs::remove_file(dir.join("run-hidden.vbs"));
    }
    crate::log::write("BT auto-connect task removed");
    Ok("removed".into())
}

pub fn is_installed() -> bool {
    schtasks(&["/Query", "/TN", TASK_NAME]).is_ok_and(|(ok, _)| ok)
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
