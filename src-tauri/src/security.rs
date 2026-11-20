// Security guards shared by file-touching commands.
//
// The webview is local, but a bug or an injected script must not be able to
// read arbitrary user files through our IPC surface. Every path accepted from
// the frontend is canonicalised and checked against a small allow-list.

use crate::settings::Settings;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Maximum base64 payload accepted by image commands (48 MB decoded-ish).
pub const MAX_B64_LEN: usize = 64 * 1024 * 1024;

/// Sanity bound for decoded screenshots (guards against decompression bombs).
pub const MAX_IMAGE_PIXELS: u32 = 100_000_000;

pub fn check_b64_size(data: &str) -> Result<(), String> {
    if data.len() > MAX_B64_LEN {
        return Err("Image payload is too large".into());
    }
    Ok(())
}

pub fn check_dimensions(width: u32, height: u32) -> Result<(), String> {
    let pixels = width
        .checked_mul(height)
        .ok_or("Image dimensions are invalid")?;
    if pixels > MAX_IMAGE_PIXELS {
        return Err("Image dimensions are too large".into());
    }
    Ok(())
}

/// Directories the frontend is allowed to touch: capture temp dir, thumbnail
/// cache, configured save folder and the default save folder.
fn allowed_roots(app: &AppHandle) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(local) = app.path().app_local_data_dir() {
        roots.push(local.join("temp"));
        roots.push(local.join("thumbnails"));
    }
    {
        let state = app.state::<Mutex<Settings>>();
        let locked = state.lock();
        if let Ok(settings) = locked {
            roots.push(crate::storage::save_dir(&*settings));
        }
    }
    roots.push(crate::storage::default_save_dir());
    roots
}

/// Reject obviously unsafe save locations (Windows/Program Files/drive root).
/// Users may still choose any normal folder, including on another drive.
pub fn validate_save_dir(path: &str) -> Result<(), String> {
    if path.is_empty() || path.len() > 4096 {
        return Err("Invalid save directory".into());
    }
    let p = Path::new(path);
    if !p.is_absolute() || p.components().count() < 3 {
        return Err("Choose a subfolder, not a drive root".into());
    }
    let lower = path.to_ascii_lowercase().replace('/', "\\");
    for forbidden in [
        "\\windows\\",
        "\\program files",
        "\\program files (x86)",
        "\\programdata\\",
        "\\system32\\",
        "\\syswow64\\",
    ] {
        if lower.contains(forbidden) {
            return Err("That system folder cannot be used for screenshots".into());
        }
    }
    Ok(())
}

/// Resolve and validate a path supplied by the frontend.
pub fn validate_path(app: &AppHandle, path: &str) -> Result<PathBuf, String> {
    if path.is_empty() || path.len() > 4096 {
        return Err("Invalid path".into());
    }
    let requested = Path::new(path);
    let canonical = std::fs::canonicalize(requested)
        .map_err(|_| "File or folder does not exist".to_string())?;

    for root in allowed_roots(app) {
        if let Ok(canonical_root) = std::fs::canonicalize(&root) {
            if canonical.starts_with(&canonical_root) {
                return Ok(canonical);
            }
        }
    }
    Err("Access to this path is not allowed".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_payloads() {
        assert!(check_b64_size("ok").is_ok());
        assert!(check_b64_size(&"x".repeat(MAX_B64_LEN + 1)).is_err());
    }

    #[test]
    fn rejects_absurd_dimensions() {
        assert!(check_dimensions(1920, 1080).is_ok());
        assert!(check_dimensions(20_000, 20_000).is_err());
        assert!(check_dimensions(u32::MAX, 2).is_err());
    }
}
