// Screenshot history: scans the save directory, exposes metadata and cached
// thumbnails. No database — the directory is the source of truth.

use crate::settings::Settings;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub file_name: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_ms: i64,
    pub width: u32,
    pub height: u32,
}

const EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn list(save_dir: &Path, limit: usize) -> Vec<HistoryItem> {
    let mut items = Vec::new();
    let Ok(entries) = std::fs::read_dir(save_dir) else {
        return items;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || !is_image(&path) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let (width, height) = image::image_dimensions(&path).unwrap_or((0, 0));
        items.push(HistoryItem {
            file_name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            path: path.to_string_lossy().to_string(),
            size_bytes: meta.len(),
            modified_ms,
            width,
            height,
        });
    }
    items.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    items.truncate(limit);
    items
}

fn thumb_cache_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_local_data_dir()
        .expect("app local data dir")
        .join("thumbnails");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Generate (or fetch from cache) a 320px-max thumbnail; returns its path.
pub fn thumbnail(app: &AppHandle, image_path: &str) -> Result<PathBuf, String> {
    let src = Path::new(image_path);
    if !src.is_file() {
        return Err("File not found".into());
    }
    let meta = std::fs::metadata(src).map_err(|e| e.to_string())?;
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let stem = src
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "thumb".into());
    let cache = thumb_cache_dir(app).join(format!("{stem}-{modified_ms}.png"));
    if cache.is_file() {
        return Ok(cache);
    }
    let img = image::open(src).map_err(|e| format!("Failed to read image: {e}"))?;
    let thumb = if img.width().max(img.height()) > 320 {
        img.thumbnail(320, 320)
    } else {
        img
    };
    thumb
        .save(&cache)
        .map_err(|e| format!("Failed to write thumbnail: {e}"))?;
    Ok(cache)
}

/// Send a file to the Recycle Bin (FO_DELETE with FOF_ALLOWUNDO).
pub fn recycle(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        SHFILEOPSTRUCTW,
    };

    let full = path.canonicalize().map_err(|e| e.to_string())?;
    let mut wide: Vec<u16> = full
        .as_os_str()
        .encode_wide()
        .collect();
    wide.push(0); // SHFileOperationW expects a double-null-terminated list.
    wide.push(0);

    let mut op = SHFILEOPSTRUCTW {
        hwnd: std::ptr::null_mut(),
        wFunc: FO_DELETE as u32,
        pFrom: wide.as_ptr(),
        pTo: std::ptr::null(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16,
        fAnyOperationsAborted: 0,
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: std::ptr::null(),
    };
    let hr = unsafe { SHFileOperationW(&mut op) };
    if hr != 0 || op.fAnyOperationsAborted != 0 {
        return Err("Could not move the file to the Recycle Bin".into());
    }
    Ok(())
}

/// Reveal a file in Explorer (selects it).
pub fn reveal_in_explorer(path: &str) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(Path::new(path)).map_err(|e| e.to_string())
}

/// Ensure the save dir exists; returns the resolved dir.
pub fn ensure_save_dir(settings: &Settings) -> PathBuf {
    let dir = crate::storage::save_dir(settings);
    let _ = std::fs::create_dir_all(&dir);
    dir
}
