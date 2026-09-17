//! Plain-text log with tail rotation: `%APPDATA%\DroidBridge\droidbridge.log`,
//! trimmed to the last ~400 lines on write (same contract as the PS
//! original). Best-effort: logging failures are swallowed.

use std::fs;

const MAX_LINES: usize = 400;

pub fn log_path() -> Option<std::path::PathBuf> {
    let appdata = std::env::var("APPDATA").ok()?;
    Some(
        std::path::PathBuf::from(appdata)
            .join("DroidBridge")
            .join("droidbridge.log"),
    )
}

/// Append one timestamped line, rotating the file if it grew past
/// `MAX_LINES`.
pub fn write(msg: &str) {
    let Some(path) = log_path() else { return };
    let _ = path.parent().map(fs::create_dir_all);
    if let Ok(existing) = fs::read_to_string(&path)
        && existing.lines().count() >= MAX_LINES
    {
        let lines: Vec<&str> = existing.lines().collect();
        let tail: String = lines[lines.len() - MAX_LINES / 2..].join("\n");
        let _ = fs::write(&path, format!("{tail}\n"));
    }
    let line = format!("{} {msg}", timestamp());
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        use std::io::Write;
        let _ = writeln!(file, "{line}");
    }
}

/// `2026-09-17T12:34:56+05:00` — local time, ISO-ish, zero deps.
fn timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d, h, mi, s, _) = civil_from_unix(secs as i64);
    // local offset is not worth a chrono dep; log UTC with marker
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Days-to-civil algorithm (Howard Hinnant) for UTC breakdown.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let dow = (days + 4).rem_euclid(7) as u32; // 1970-01-01 was Thursday
    (y, m, d, h as u32, mi as u32, s as u32, dow)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_epoch_breakdown() {
        let (y, m, d, h, mi, s, dow) = civil_from_unix(0);
        assert_eq!((y, m, d, h, mi, s), (1970, 1, 1, 0, 0, 0));
        assert_eq!(dow, 4); // Thursday
    }

    #[test]
    fn known_date() {
        // 2026-09-17T00:00:00Z
        let (y, m, d, h, _, _, _) = civil_from_unix(1_789_603_200);
        assert_eq!((y, m, d, h), (2026, 9, 17, 0));
    }
}
