//! Port cache: `%APPDATA%\DroidBridge\portcache.json`.
//!
//! Format is shared with the PowerShell original. Caveat: PS
//! `ConvertTo-Json` on a single-element array emits a bare number
//! (`12345` instead of `[12345]`), so both shapes are accepted when
//! reading. Writing always uses a proper JSON array, which PS reads fine.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

pub fn cache_path() -> Result<PathBuf> {
    let appdata = std::env::var("APPDATA").context("%APPDATA% is not set")?;
    Ok(PathBuf::from(appdata)
        .join("DroidBridge")
        .join("portcache.json"))
}

pub fn load() -> Vec<u16> {
    let Ok(path) = cache_path() else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    parse(&raw).unwrap_or_default()
}

/// Parse both `[12345]` and the bare `12345` shapes PS produces.
pub fn parse(raw: &str) -> Option<Vec<u16>> {
    let raw = raw.trim();
    if let Ok(ports) = serde_json::from_str::<Vec<u16>>(raw) {
        return Some(ports);
    }
    raw.parse::<u16>().ok().map(|p| vec![p])
}

pub fn save(ports: &[u16]) -> Result<()> {
    let path = cache_path()?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(&path, serde_json::to_string(ports)?)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_array_form() {
        assert_eq!(parse("[12345, 40000]"), Some(vec![12345, 40000]));
    }

    #[test]
    fn parses_ps_bare_number_form() {
        // single-element PS ConvertTo-Json output has no brackets
        assert_eq!(parse("12345\n"), Some(vec![12345]));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse("nonsense"), None);
    }
}
