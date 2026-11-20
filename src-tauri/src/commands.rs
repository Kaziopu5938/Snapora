// Tauri commands: the IPC surface exposed to the frontend.

use crate::capture::{gdi, overlay};
use crate::clipboard;
use crate::error::AppResult;
use crate::history::{self, HistoryItem};
use crate::settings::{self, CaptureMode, ImageFormat, Settings};
use crate::storage::{self, SavedInfo};
use crate::shortcuts::{self, ShortcutReport};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

// ---------------------------------------------------------------- capture ---

#[tauri::command]
pub async fn start_capture(
    app: AppHandle,
    mode: CaptureMode,
    monitor_index: Option<usize>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || crate::capture::begin_capture(&app, mode, monitor_index))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_capture(app: AppHandle) -> Result<(), String> {
    overlay::cancel(&app)
}

/// Called by the frontend *after* it has switched to the editor view, so the
/// window always opens on the editor rather than a stale Home screen.
#[tauri::command]
pub fn reveal_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn discard_capture(temp_path: String) -> Result<(), String> {
    crate::capture::discard_capture(&temp_path);
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRect {
    pub id: usize,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorOut {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub primary: bool,
}

impl From<&gdi::MonitorInfo> for MonitorOut {
    fn from(m: &gdi::MonitorInfo) -> Self {
        Self {
            index: m.index,
            x: m.x,
            y: m.y,
            width: m.width,
            height: m.height,
            dpi: m.dpi,
            primary: m.primary,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayInfo {
    pub kind: String,
    pub monitor: MonitorOut,
    pub bitmap_path: String,
    pub windows: Vec<WindowRect>,
}

/// Called by each overlay window; the caller is identified by its label
/// (`overlay-<monitorIndex>`).
#[tauri::command]
pub fn overlay_info(window: WebviewWindow) -> Result<OverlayInfo, String> {
    let app = window.app_handle();
    let idx: usize = window
        .label()
        .trim_start_matches("overlay-")
        .parse()
        .map_err(|_| "bad overlay label".to_string())?;
    let session: State<'_, Arc<Mutex<Option<overlay::CaptureSession>>>> = app.state();
    let guard = session.lock().map_err(|_| "session poisoned".to_string())?;
    let Some(s) = guard.as_ref() else {
        return Err("No capture session".into());
    };
    let Some(m) = s.monitors.iter().find(|m| m.index == idx) else {
        return Err("Unknown monitor for overlay".into());
    };
    let bitmap_path = s
        .temp_dir
        .join(format!("monitor-{}.png", m.index))
        .to_string_lossy()
        .to_string();
    let windows = s
        .windows
        .iter()
        .filter(|w| {
            // Only windows intersecting this monitor.
            w.x < m.x + m.width && w.x + w.w > m.x && w.y < m.y + m.height && w.y + w.h > m.y
        })
        .map(|w| WindowRect {
            id: w.id,
            title: w.title.clone(),
            x: w.x,
            y: w.y,
            w: w.w,
            h: w.h,
        })
        .collect();
    Ok(OverlayInfo {
        kind: match s.kind {
            overlay::InteractiveKind::Region => "region".into(),
            overlay::InteractiveKind::Freeform => "freeform".into(),
            overlay::InteractiveKind::WindowSelect => "window".into(),
        },
        monitor: MonitorOut {
            index: m.index,
            x: m.x,
            y: m.y,
            width: m.width,
            height: m.height,
            dpi: m.dpi,
            primary: m.primary,
        },
        bitmap_path,
        windows,
    })
}

/// Window highlight lookup for WindowSelect mode (global physical px).
#[tauri::command]
pub fn overlay_window_at(app: AppHandle, x: i32, y: i32) -> Result<Option<WindowRect>, String> {
    let session: State<'_, Arc<Mutex<Option<overlay::CaptureSession>>>> = app.state();
    let guard = session.lock().map_err(|_| "session poisoned".to_string())?;
    let Some(s) = guard.as_ref() else {
        return Ok(None);
    };
    Ok(overlay::window_at(s, x, y).map(|w| WindowRect {
        id: w.id,
        title: w.title.clone(),
        x: w.x,
        y: w.y,
        w: w.w,
        h: w.h,
    }))
}

/// Complete a selection. Coordinates are global physical pixels.
#[tauri::command]
pub async fn overlay_finish(
    app: AppHandle,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<i32>,
    h: Option<i32>,
    points: Option<Vec<(f64, f64)>>,
    window_id: Option<usize>,
) -> Result<(), String> {
    let rect = match (x, y, w, h) {
        (Some(x), Some(y), Some(w), Some(h)) => Some((x, y, w, h)),
        _ => None,
    };
    tauri::async_runtime::spawn_blocking(move || overlay::finish(&app, rect, points, window_id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_monitors() -> Vec<MonitorOut> {
    gdi::enumerate_monitors().iter().map(MonitorOut::from).collect()
}

// --------------------------------------------------------------- clipboard ---

#[tauri::command]
pub fn copy_image(data_b64: String) -> Result<(), String> {
    crate::security::check_b64_size(&data_b64)?;
    clipboard::copy_image_png_b64(&data_b64)
}

/// Read an approved image file from disk and return a base64-encoded PNG. The
/// editor uses this instead of the asset protocol so the canvas is never
/// tainted; the path is confined to capture/temp/save/thumbnail directories.
#[tauri::command]
pub fn read_image_b64(app: AppHandle, path: String) -> Result<String, String> {
    use base64::Engine;
    use image::ImageEncoder;
    let path = crate::security::validate_path(&app, &path)?;
    let img = image::open(&path).map_err(|e| format!("Failed to read image: {e}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    crate::security::check_dimensions(w, h)?;
    let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);
    let enc = image::codecs::png::PngEncoder::new(&mut out);
    enc.write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(&out))
}

// ----------------------------------------------------------------- storage ---

#[tauri::command]
pub fn get_save_dir(app: AppHandle) -> Result<String, String> {
    let s = app.state::<Mutex<Settings>>();
    let guard = s.lock().map_err(|_| "settings poisoned".to_string())?;
    Ok(history::ensure_save_dir(&guard).to_string_lossy().to_string())
}

#[tauri::command]
pub async fn choose_save_dir(app: AppHandle) -> Result<Option<String>, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let picked = handle
            .dialog()
            .file()
            .set_title("Choose default save folder")
            .blocking_pick_folder();
        Ok(picked.and_then(|p| p.into_path().ok()).map(|pb: PathBuf| {
            pb.to_string_lossy().to_string()
        }))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn save_image(
    app: AppHandle,
    data_b64: String,
    format: Option<ImageFormat>,
) -> Result<SavedInfo, String> {
    crate::security::check_b64_size(&data_b64)?;
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> AppResult<SavedInfo> {
        let state = app2.state::<Mutex<Settings>>();
        let (dir, fmt) = {
            let s = state.lock().map_err(|_| "settings poisoned".to_string())?;
            (history::ensure_save_dir(&s), format.unwrap_or(s.format))
        };
        let stem = overlay::suggested_file_name(chrono::Utc::now().timestamp_millis());
        let bytes = storage::transcode_b64_png(&data_b64, fmt)?;
        storage::save_bytes(&dir, &stem, fmt, &bytes)
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("history://changed", ());
    Ok(result)
}

#[tauri::command]
pub async fn save_image_as(
    app: AppHandle,
    data_b64: String,
    suggested_name: String,
) -> Result<Option<SavedInfo>, String> {
    crate::security::check_b64_size(&data_b64)?;
    if suggested_name.is_empty()
        || suggested_name.len() > 255
        || suggested_name.contains('\\')
        || suggested_name.contains('/')
    {
        return Err("Invalid file name".into());
    }
    let handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<Option<SavedInfo>, String> {
        let picked = handle
            .dialog()
            .file()
            .set_title("Save screenshot as")
            .add_filter("Image files", &["png", "jpg", "jpeg", "webp"])
            .set_file_name(&suggested_name)
            .blocking_save_file();
        let Some(picked) = picked else {
            return Ok(None); // user cancelled
        };
        let path: PathBuf = picked
            .into_path()
            .map_err(|e| e.to_string())?;
        // Ensure an extension; default to PNG.
        let path = if path.extension().is_none() {
            path.with_extension("png")
        } else {
            path
        };
        let Some(fmt) = storage::extension_format(&path) else {
            return Err("Unsupported file type — use .png, .jpg or .webp".into());
        };
        let bytes = storage::transcode_b64_png(&data_b64, fmt)?;
        let saved = storage::write_image_file(&path, &bytes)?;
        Ok(Some(SavedInfo {
            path: saved.to_string_lossy().to_string(),
            file_name: saved
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            dir: saved
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        }))
    })
    .await
    .map_err(|e| e.to_string())?;
    if result.is_ok() {
        let _ = app.emit("history://changed", ());
    }
    result
}

// ----------------------------------------------------------------- history ---

#[tauri::command]
pub async fn list_history(app: AppHandle) -> Result<Vec<HistoryItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<Mutex<Settings>>();
        let guard = state.lock().map_err(|_| "settings poisoned".to_string())?;
        let dir = history::ensure_save_dir(&guard);
        Ok(history::list(&dir, 500))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_thumbnail(app: AppHandle, path: String) -> Result<String, String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = crate::security::validate_path(&app2, &path)?;
        history::thumbnail(&app2, &path.to_string_lossy()).map(|p| p.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn delete_history_item(app: AppHandle, path: String) -> Result<(), String> {
    let app2 = app.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let p = crate::security::validate_path(&app2, &path)?;
        if !p.is_file() {
            return Err("File not found".to_string());
        }
        history::recycle(&p)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|inner| inner);
    let _ = app.emit("history://changed", ());
    outcome
}

#[tauri::command]
pub fn reveal_in_explorer(app: AppHandle, path: String) -> Result<(), String> {
    let path = crate::security::validate_path(&app, &path)?;
    history::reveal_in_explorer(&path.to_string_lossy())
}

#[tauri::command]
pub fn open_path(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = crate::security::validate_path(&app, &path)?;
    let path = path.to_string_lossy().into_owned();
    app.opener()
        .open_path(path, None::<&str>)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- settings ---

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let state = app.state::<Mutex<Settings>>();
    let guard = state.lock().map_err(|_| "settings poisoned".to_string())?;
    Ok(guard.clone())
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    if let Some(dir) = &settings.save_dir {
        crate::security::validate_save_dir(dir)?;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        {
            let state = handle.state::<Mutex<Settings>>();
            let mut guard = state.lock().map_err(|_| "settings poisoned".to_string())?;
            *guard = settings.clone();
            settings::persist(&handle, &guard)?;
        }
        apply_side_effects(&handle, &settings);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Apply settings that touch the OS: autostart, hotkeys, theme, save dir.
pub fn apply_side_effects(app: &AppHandle, s: &Settings) {
    use tauri_plugin_autostart::ManagerExt as _;
    // Start with Windows.
    let autostart = app.autolaunch();
    let _ = if s.start_with_windows {
        autostart.enable()
    } else {
        autostart.disable()
    };
    // Save dir exists.
    let _ = std::fs::create_dir_all(storage::save_dir(s));
    // Give the physical Print Screen key to Snapora instead of Windows Snipping Tool.
    let _ = shortcuts::set_os_snipping_setting(!s.take_over_print_screen);
    // Global shortcuts (re-registered after the OS setting changes).
    shortcuts::register_all(app, s);
    // Window theme.
    if let Some(win) = app.get_webview_window("main") {
        let theme = match s.theme {
            settings::Theme::Light => Some(tauri::Theme::Light),
            settings::Theme::Dark => Some(tauri::Theme::Dark),
            settings::Theme::System => None,
        };
        let _ = win.set_theme(theme);
    }
    let _ = app.emit("settings://applied", s);
}

#[tauri::command]
pub fn get_shortcut_report(app: AppHandle) -> ShortcutReport {
    app.try_state::<shortcuts::SharedReport>()
        .and_then(|s| s.lock().ok().map(|r| r.clone()))
        .unwrap_or_default()
}

#[tauri::command]
pub fn open_windows_settings(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url("ms-settings:easeofaccess-keyboard", None::<&str>)
        .map_err(|e| e.to_string())
}
