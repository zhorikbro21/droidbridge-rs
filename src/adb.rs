//! adb.exe discovery + command wrappers + the connect flow
//! (cache → mDNS → full scan), mirroring the PowerShell original.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::Config;
use crate::portcache;
use crate::scanner;

/// A candidate adb endpoint discovered via mDNS.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Endpoint {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
}

/// Parse `adb devices` output into (serial, state) pairs.
/// Unknown states and junk lines are skipped, like the PS original
/// (which only matches lines ending in `device`).
pub fn parse_devices(out: &str) -> Vec<(String, DeviceState)> {
    out.lines()
        .filter_map(|line| {
            let line = line.trim_end();
            if line.is_empty() || line.starts_with("List of devices") {
                return None;
            }
            let mut parts = line.split_whitespace();
            let serial = parts.next()?.to_string();
            match parts.next()? {
                "device" => Some((serial, DeviceState::Device)),
                "offline" => Some((serial, DeviceState::Offline)),
                "unauthorized" => Some((serial, DeviceState::Unauthorized)),
                _ => None,
            }
        })
        .collect()
}

/// First serial currently in `device` state (e.g. `192.168.1.5:12345`),
/// or None.
pub fn connected_device(adb: &Path) -> Result<Option<String>> {
    let out = run_adb(adb, &["devices"])?;
    Ok(parse_devices(&out)
        .into_iter()
        .find(|(_, st)| *st == DeviceState::Device)
        .map(|(serial, _)| serial))
}

/// Extract all `_adb-tls-connect` endpoints from `adb mdns services`
/// output. Deliberately not filtered by deviceHost: the phone may
/// advertise on a hotspot IP while the config names its tailnet IP.
pub fn parse_mdns_endpoints(out: &str) -> Vec<Endpoint> {
    out.lines()
        .filter(|l| l.contains("_adb-tls-connect"))
        .filter_map(|l| {
            // last token looks like 10.0.0.5:12345
            let tail = l.split_whitespace().last()?;
            let (host, port) = tail.rsplit_once(':')?;
            let port: u16 = port.parse().ok()?;
            host.parse::<std::net::IpAddr>().ok()?;
            Some(Endpoint {
                host: host.to_string(),
                port,
            })
        })
        .collect()
}

/// Endpoints via the local mdns-sd daemon (fallback when the adb mDNS
/// backend is unhealthy). Bounded by `deadline`; unfiltered by IP for
/// the same reason as [`parse_mdns_endpoints`].
pub fn native_mdns_endpoints(deadline: Duration) -> Vec<Endpoint> {
    let mut found = Vec::new();
    let Ok(daemon) = mdns_sd::ServiceDaemon::new() else {
        return found;
    };
    let Ok(receiver) = daemon.browse("_adb-tls-connect._tcp.local.") else {
        return found;
    };
    let deadline = std::time::Instant::now() + deadline;
    while let Ok(event) = receiver.recv_deadline(deadline) {
        match event {
            mdns_sd::ServiceEvent::ServiceResolved(info) => {
                // Prefer usable IPv4 addresses; skip link-local ones.
                for addr in info.get_addresses_v4() {
                    if !addr.is_link_local() {
                        found.push(Endpoint {
                            host: addr.to_string(),
                            port: info.get_port(),
                        });
                    }
                }
            }
            mdns_sd::ServiceEvent::SearchStopped(_) => break,
            _ => {}
        }
    }
    let _ = daemon.shutdown();
    found
}

fn run_adb(adb: &Path, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new(adb);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .with_context(|| format!("running {}", adb.display()))?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `adb pair ip:port code` — returns the combined stdout/stderr message.
pub fn pair(adb: &Path, ip: &str, port: u16, code: &str) -> Result<String> {
    let target = format!("{ip}:{port}");
    let mut cmd = Command::new(adb);
    cmd.args(["pair", &target, code]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd
        .output()
        .with_context(|| format!("running {} pair", adb.display()))?;
    let mut text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&out.stderr).trim().to_string();
    }
    if !out.status.success() {
        anyhow::bail!("{text}");
    }
    Ok(text)
}

/// `adb connect host:port`, then verify it reached `device` state
/// (an open-but-wrong port parks as `offline` and is rejected).
/// Disconnects on failure so zombie entries do not accumulate.
fn try_connect(adb: &Path, host: &str, port: u16) -> Result<Option<String>> {
    let target = format!("{host}:{port}");
    run_adb(adb, &["connect", &target])?;
    std::thread::sleep(Duration::from_secs(2));
    match connected_device(adb)? {
        Some(serial) => Ok(Some(serial)),
        None => {
            let _ = run_adb(adb, &["disconnect", &target]);
            Ok(None)
        }
    }
}

/// The full connect flow: existing device → cached port → mDNS →
/// (optionally) full range scan. Returns the connected serial on success.
pub fn connect_phone(adb: &Path, cfg: &Config, allow_scan: bool) -> Result<Option<String>> {
    if let Some(serial) = connected_device(adb)? {
        return Ok(Some(serial));
    }

    if cfg.device_host.is_empty() {
        anyhow::bail!("no phone IP configured - set deviceHost in config");
    }
    let host = cfg.device_host.clone();

    let _ = run_adb(adb, &["disconnect"]);

    // Tier 1 + 2: cached ports on the configured host, plus every
    // mDNS-advertised endpoint (the phone may live on another interface).
    let mut candidates: Vec<Endpoint> = portcache::load()
        .into_iter()
        .map(|port| Endpoint {
            host: host.clone(),
            port,
        })
        .collect();
    let mdns_out = run_adb(adb, &["mdns", "services"]).unwrap_or_default();
    candidates.extend(parse_mdns_endpoints(&mdns_out));
    candidates.extend(native_mdns_endpoints(Duration::from_secs(3)));
    candidates.sort_unstable();
    candidates.dedup();
    crate::log::write(&format!(
        "candidates from cache+mdns: {}",
        candidates
            .iter()
            .map(|e| format!("{}:{}", e.host, e.port))
            .collect::<Vec<_>>()
            .join(", ")
    ));

    for ep in &candidates {
        if let Some(serial) = try_connect(adb, &ep.host, ep.port)? {
            portcache::save(&[ep.port])?;
            return Ok(Some(serial));
        }
    }

    // Tier 3: full ephemeral-range scan on the configured host — but
    // only if the host is even reachable; a dead host must fail in
    // seconds, not after two full 28k-port sweeps.
    if allow_scan {
        let probe_port = portcache::load().first().copied().unwrap_or(53);
        let reachable = scanner::tcp_probe(&host, probe_port, Duration::from_millis(1200));
        if reachable {
            crate::log::write(&format!(
                "full port scan on {host} ({}-{}) started",
                cfg.port_min, cfg.port_max
            ));
            for port in scanner::scan_open_ports_blocking(&host, cfg.port_min, cfg.port_max) {
                if let Some(serial) = try_connect(adb, &host, port)? {
                    portcache::save(&[port])?;
                    return Ok(Some(serial));
                }
            }
        } else {
            crate::log::write(&format!("host {host} unreachable, skipping scan"));
        }
    }

    Ok(None)
}

/// Is a scrcpy.exe process already running?
pub fn scrcpy_running() -> bool {
    let Ok(out) = Command::new("tasklist.exe")
        .args(["/FI", "IMAGENAME eq scrcpy.exe", "/FO", "CSV", "/NH"])
        .output()
    else {
        return false;
    };
    let text = String::from_utf8_lossy(&out.stdout);
    text.to_lowercase().contains("scrcpy.exe")
}

/// Launch scrcpy unless an instance is already up (never spawn a second
/// mirror window).
pub fn launch_scrcpy_once(cfg: &Config) -> Result<()> {
    if scrcpy_running() {
        return Ok(());
    }
    let Some(scrcpy) = resolve_scrcpy(cfg) else {
        anyhow::bail!("scrcpy.exe not found - set scrcpyPath in config");
    };
    Command::new(&scrcpy)
        .spawn()
        .with_context(|| format!("launching {}", scrcpy.display()))?;
    Ok(())
}

/// The folder containing the running exe — first stop for portable
/// setups where adb/scrcpy ship next to droidbridge_rs.exe.
fn own_dir(exe_name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(exe_name);
    candidate.is_file().then_some(candidate)
}

/// Locate adb.exe: config path → own folder (portable zip) → sibling of
/// scrcpy (version-compatible with the running scrcpy adb server) → SDK
/// platform-tools → PATH → common install globs.
pub fn resolve_adb(cfg: &Config) -> Option<PathBuf> {
    if !cfg.adb_path.is_empty() {
        let p = PathBuf::from(&cfg.adb_path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = own_dir("adb.exe") {
        return Some(p);
    }
    if let Some(scrcpy) = resolve_scrcpy(cfg)
        && let Some(dir) = scrcpy.parent()
    {
        let sibling = dir.join("adb.exe");
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let p = PathBuf::from(local).join(r"Android\Sdk\platform-tools\adb.exe");
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = find_in_path("adb.exe") {
        return Some(p);
    }
    find_by_globs("adb.exe", &["scrcpy", "scrcpy-win"])
}

/// Locate scrcpy.exe: config path → own folder (portable zip) → PATH →
/// common install globs.
pub fn resolve_scrcpy(cfg: &Config) -> Option<PathBuf> {
    if !cfg.scrcpy_path.is_empty() {
        let p = PathBuf::from(&cfg.scrcpy_path);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(p) = own_dir("scrcpy.exe") {
        return Some(p);
    }
    if let Some(p) = find_in_path("scrcpy.exe") {
        return Some(p);
    }
    find_by_globs("scrcpy.exe", &["scrcpy", "scrcpy-win"])
}

fn find_in_path(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(exe))
        .find(|p| p.is_file())
}

/// Match patterns like `%USERPROFILE%\Downloads\scrcpy*\adb.exe` and
/// `C:\scrcpy*\adb.exe`: the `prefixes` are directory-name prefixes tried
/// one and two levels deep under Downloads and `C:\`.
fn find_by_globs(exe: &str, prefixes: &[&str]) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE") {
        roots.push(PathBuf::from(home).join("Downloads"));
    }
    roots.push(PathBuf::from(r"C:\"));

    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !prefixes.iter().any(|p| name.starts_with(p)) {
                continue;
            }
            for candidate in [
                entry.path().join(exe),
                entry.path().join("scrcpy").join(exe),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "List of devices attached\r\n\
        192.0.2.10:12345\tdevice\r\n\
        10.0.0.8:40000\toffline\r\n\
        10.0.0.9:40001\tunauthorized\r\n\
        emulator-5554\tdevice\r\n\
        \r\n";

    #[test]
    fn parses_device_states() {
        let devs = parse_devices(SAMPLE);
        assert_eq!(devs.len(), 4);
        assert_eq!(devs[0], ("192.0.2.10:12345".into(), DeviceState::Device));
        assert_eq!(devs[1], ("10.0.0.8:40000".into(), DeviceState::Offline));
        assert_eq!(
            devs[2],
            ("10.0.0.9:40001".into(), DeviceState::Unauthorized)
        );
        assert_eq!(devs[3], ("emulator-5554".into(), DeviceState::Device));
    }

    #[test]
    fn skips_junk_lines() {
        let out = "List of devices attached\r\n\r\nbogus line without state\r\n";
        assert!(parse_devices(out).is_empty());
    }

    #[test]
    fn first_device_serial() {
        let devs = parse_devices(SAMPLE);
        let first = devs
            .iter()
            .find(|(_, st)| *st == DeviceState::Device)
            .map(|(s, _)| s.clone());
        assert_eq!(first.as_deref(), Some("192.0.2.10:12345"));
    }

    #[test]
    fn parses_mdns_endpoints_unfiltered() {
        let out = "List of discovered mdns services\r\n\
            \tadb-XXX\t_adb-tls-connect._tcp\t12512\t192.168.1.5:12512\r\n\
            \tadb-XXX\t_adb-tls-pairing._tcp\t36623\t192.168.1.5:36623\r\n\
            \tadb-YYY\t_adb-tls-connect._tcp\t39001\t192.0.2.20:39001\r\n";
        let eps = parse_mdns_endpoints(out);
        assert_eq!(
            eps,
            vec![
                Endpoint {
                    host: "192.168.1.5".into(),
                    port: 12512
                },
                Endpoint {
                    host: "192.0.2.20".into(),
                    port: 39001
                },
            ]
        );
        // pairing services must never appear
        assert!(!eps.iter().any(|e| e.port == 36623));
    }
}
