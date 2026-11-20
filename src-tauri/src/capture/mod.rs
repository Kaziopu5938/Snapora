// Capture orchestration: entry points used by global shortcuts, the tray
// menu and the frontend commands.

pub mod gdi;
pub mod overlay;
pub mod wininfo;

pub use crate::settings::CaptureMode;

use crate::error::AppResult;
use overlay::InteractiveKind;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// Payload emitted to the main window when a capture is ready to edit.
#[allow(dead_code)] // serde shape kept as the documented event contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureReady {
    #[serde(rename = "tempPath")]
    pub temp_path: String,
    #[serde(rename = "fileName")]
    pub file_name: String,
    pub width: u32,
    pub height: u32,
    pub source: &'static str,
}

static BUSY: AtomicBool = AtomicBool::new(false);

fn interactive_kind(mode: CaptureMode) -> Option<InteractiveKind> {
    match mode {
        CaptureMode::Region => Some(InteractiveKind::Region),
        CaptureMode::Freeform => Some(InteractiveKind::Freeform),
        CaptureMode::WindowSelect => Some(InteractiveKind::WindowSelect),
        _ => None,
    }
}

/// Central capture entry. Returns Err(message) on failure; success emits
/// `capture://ready` (quick modes) or opens overlays (interactive modes).
pub fn begin_capture(
    app: &AppHandle,
    mode: CaptureMode,
    monitor_index: Option<usize>,
) -> Result<(), String> {
    if BUSY.swap(true, Ordering::SeqCst) {
        return Err("A capture is already in progress".into());
    }
    let result = run_capture(app, mode, monitor_index);
    BUSY.store(false, Ordering::SeqCst);
    result
}

fn run_capture(
    app: &AppHandle,
    mode: CaptureMode,
    monitor_index: Option<usize>,
) -> Result<(), String> {
    let (delay_secs, include_cursor, fullscreen_all) = {
        let state = app.state::<Mutex<crate::settings::Settings>>();
        let s = state.lock().map_err(|_| "settings poisoned")?;
        (s.delay_secs, s.include_cursor, s.fullscreen_all_monitors)
    };

    if let Some(kind) = interactive_kind(mode) {
        return overlay::start_interactive(app, kind, delay_secs, include_cursor);
    }

    // --- Quick captures ------------------------------------------------------
    let main = app.get_webview_window("main");
    let was_visible = main
        .as_ref()
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    if was_visible {
        if let Some(w) = &main {
            let _ = w.hide();
        }
        std::thread::sleep(std::time::Duration::from_millis(160));
    }
    if delay_secs > 0 {
        std::thread::sleep(std::time::Duration::from_secs(delay_secs as u64));
    }

    let captured = (|| -> AppResult<gdi::Bitmap> {
        match mode {
            CaptureMode::Fullscreen => {
                if fullscreen_all {
                    gdi::capture_virtual_screen(include_cursor)
                } else {
                    let monitors = gdi::enumerate_monitors();
                    let primary = monitors
                        .iter()
                        .find(|m| m.primary)
                        .cloned()
                        .unwrap_or_else(|| monitors.first().cloned().expect("at least one monitor"));
                    gdi::capture_rect(primary.x, primary.y, primary.width, primary.height, include_cursor)
                }
            }
            CaptureMode::AllMonitors => gdi::capture_virtual_screen(include_cursor),
            CaptureMode::Monitor => {
                let monitors = gdi::enumerate_monitors();
                let idx = monitor_index
                    .or_else(|| {
                        monitors.iter().find(|m| m.primary).map(|m| m.index)
                    })
                    .ok_or("No monitor found")?;
                let m = monitors
                    .into_iter()
                    .find(|m| m.index == idx)
                    .ok_or("Monitor not found")?;
                gdi::capture_rect(m.x, m.y, m.width, m.height, include_cursor)
            }
            CaptureMode::ActiveWindow => {
                let hwnd = wininfo::foreground_window();
                if hwnd == 0 {
                    return Err("No active window".into());
                }
                wininfo::capture_window(hwnd, include_cursor)
            }
            _ => unreachable!("interactive modes handled above"),
        }
    })();

    let bitmap = match captured {
        Ok(b) => b,
        Err(e) => {
            restore_main(app, was_visible);
            return Err(format!("Screen capture failed: {e}"));
        }
    };

    let captured_at = chrono::Utc::now().timestamp_millis();
    let temp_dir = overlay::session_temp_root(app).join(format!("capture-{captured_at}"));
    let emit_result = (|| -> Result<(), String> {
        std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {e}"))?;
        let png = bitmap.encode_png()?;
        let cap_path = temp_dir.join("capture.png");
        std::fs::write(&cap_path, png).map_err(|e| format!("Failed to write capture: {e}"))?;
        let dims = (bitmap.width, bitmap.height);
        let _ = bitmap;
        // Do NOT raise the window here: the frontend switches to the editor
        // view first, then calls reveal_main_window. Showing it now would
        // briefly display the Home screen.
        let _ = app.emit(
            "capture://ready",
            serde_json::json!({
                "tempPath": cap_path.to_string_lossy(),
                "fileName": overlay::suggested_file_name(captured_at),
                "width": dims.0,
                "height": dims.1,
                "source": quick_source(mode),
            }),
        );
        Ok(())
    })();

    match emit_result {
        Ok(()) => Ok(()),
        Err(e) => {
            restore_main(app, was_visible);
            let _ = std::fs::remove_dir_all(&temp_dir);
            Err(e)
        }
    }
}

fn quick_source(mode: CaptureMode) -> &'static str {
    match mode {
        CaptureMode::Fullscreen => "fullscreen",
        CaptureMode::AllMonitors => "all-monitors",
        CaptureMode::Monitor => "monitor",
        CaptureMode::ActiveWindow => "active-window",
        _ => "capture",
    }
}

fn restore_main(app: &AppHandle, was_visible: bool) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        if was_visible {
            let _ = w.show();
        }
        let _ = w.set_focus();
    }
}

/// Delete a temp capture file (editor discard).
pub fn discard_capture(path: &str) {
    let p = std::path::Path::new(path);
    if let Some(parent) = p.parent() {
        if parent.ends_with("temp") || parent.to_string_lossy().contains("capture-") {
            let _ = std::fs::remove_dir_all(parent);
            return;
        }
    }
    let _ = std::fs::remove_file(p);
}
