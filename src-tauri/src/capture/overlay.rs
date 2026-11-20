// Interactive capture sessions: freeze the screen, spawn per-monitor overlay
// windows, and turn the user's selection into an image.

use super::gdi::{self, Bitmap, MonitorInfo};
use super::wininfo::{self, FrozenWindow};
use chrono::Local;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractiveKind {
    Region,
    Freeform,
    WindowSelect,
}

pub struct CaptureSession {
    pub kind: InteractiveKind,
    pub monitors: Vec<MonitorInfo>,
    pub composite: Bitmap,
    pub windows: Vec<FrozenWindow>,
    pub temp_dir: PathBuf,
    pub overlay_labels: Vec<String>,
    pub main_was_visible: bool,
    pub captured_at: i64,
}

pub type SharedSession = Arc<Mutex<Option<CaptureSession>>>;

/// Timestamp-based suggested filename, e.g. Snapora_2026-11-19_22-30-45
pub fn suggested_file_name(unix_ms: i64) -> String {
    let dt = chrono::DateTime::from_timestamp_millis(unix_ms)
        .map(|t| t.with_timezone(&Local))
        .unwrap_or_else(Local::now);
    format!("Snapora_{}", dt.format("%Y-%m-%d_%H-%M-%S"))
}

pub fn session_temp_root(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_local_data_dir()
        .expect("app local data dir")
        .join("temp");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Remove capture temp dirs older than 24h (best effort, at startup).
pub fn cleanup_stale_temp(app: &AppHandle) {
    let root = session_temp_root(app);
    let cutoff = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
        - 24 * 3600 * 1000;
    if let Ok(entries) = std::fs::read_dir(&root) {
        for e in entries.flatten() {
            if let Ok(meta) = e.metadata() {
                if meta.is_dir() {
                    if let Ok(modified) = meta.modified() {
                        let ms = modified
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        if ms < cutoff {
                            let _ = std::fs::remove_dir_all(e.path());
                        }
                    }
                }
            }
        }
    }
}

fn hide_main_if_visible(app: &AppHandle) -> bool {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
            return true;
        }
    }
    false
}

fn restore_main(app: &AppHandle, was_visible: bool) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        if was_visible {
            let _ = win.show();
        }
        let _ = win.set_focus();
    }
}

/// Freeze the desktop and open one overlay window per monitor.
pub fn start_interactive(
    app: &AppHandle,
    kind: InteractiveKind,
    delay_secs: u32,
    include_cursor: bool,
) -> Result<(), String> {
    let session: &SharedSession = app.state::<SharedSession>().inner();
    {
        let guard = session.lock().map_err(|_| "session poisoned")?;
        if guard.is_some() {
            return Err("A capture is already in progress".into());
        }
    }
    let main_was_visible = hide_main_if_visible(app);

    // Give the window manager a moment to actually hide the main window.
    std::thread::sleep(std::time::Duration::from_millis(160));
    if delay_secs > 0 {
        std::thread::sleep(std::time::Duration::from_secs(delay_secs as u64));
    }

    // Freeze: composite of the whole virtual desktop.
    let composite = gdi::capture_virtual_screen(include_cursor).map_err(|e| {
        restore_main(app, main_was_visible);
        format!("Screen capture failed: {e}")
    })?;
    let monitors = gdi::enumerate_monitors();
    let windows = wininfo::snapshot_windows();

    let captured_at = chrono::Utc::now().timestamp_millis();
    let temp_dir = session_temp_root(app).join(format!("capture-{captured_at}"));
    std::fs::create_dir_all(&temp_dir).map_err(|e| {
        restore_main(app, main_was_visible);
        format!("Failed to create temp dir: {e}")
    })?;

    // Per-monitor bitmaps for the overlays (crop from the composite).
    for m in &monitors {
        let part = composite.crop(m.x, m.y, m.width, m.height);
        let png = part.encode_png().map_err(|e| {
            restore_main(app, main_was_visible);
            e.to_string()
        })?;
        let path = temp_dir.join(format!("monitor-{}.png", m.index));
        std::fs::write(&path, png).map_err(|e| {
            restore_main(app, main_was_visible);
            format!("Failed to write overlay bitmap: {e}")
        })?;
    }

    let mut overlay_labels = Vec::new();
    for m in &monitors {
        overlay_labels.push(format!("overlay-{}", m.index));
    }

    *session.lock().map_err(|_| "session poisoned")? = Some(CaptureSession {
        kind,
        monitors,
        composite,
        windows,
        temp_dir,
        overlay_labels: overlay_labels.clone(),
        main_was_visible,
        captured_at,
    });

    // One frameless always-on-top window per monitor, positioned in physical
    // pixels. Built hidden, sized, then shown to avoid flashes.
    let mut created = Vec::new();
    for label in &overlay_labels {
        let idx: usize = label.trim_start_matches("overlay-").parse().unwrap_or(0);
        let Some(_m) = app
            .state::<SharedSession>()
            .inner()
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.monitors[idx].clone()))
        else {
            continue;
        };
        let builder = WebviewWindowBuilder::new(
            app,
            label.clone(),
            WebviewUrl::App("index.html".into()),
        )
        .title("Snapora Capture")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .focused(false);
        match builder.build() {
            Ok(win) => created.push(win),
            Err(e) => {
                cancel(app).ok();
                return Err(format!("Failed to open capture overlay: {e}"));
            }
        }
    }
    for win in &created {
        let idx: usize = win
            .label()
            .trim_start_matches("overlay-")
            .parse()
            .unwrap_or(0);
        let monitors = app
            .state::<SharedSession>()
            .inner()
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.monitors.clone()))
            .unwrap_or_default();
        if let Some(m) = monitors.get(idx) {
            let _ = win.set_position(PhysicalPosition::new(m.x, m.y));
            let _ = win.set_size(PhysicalSize::new(m.width as u32, m.height as u32));
        }
    }
    for win in &created {
        let _ = win.show();
    }
    // Focus the overlay of the primary monitor so it receives keyboard input.
    if let Some(primary_idx) = app
        .state::<SharedSession>()
        .inner()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|s| s.monitors.iter().find(|m| m.primary).map(|m| m.index)))
    {
        if let Some(win) = app.get_webview_window(&format!("overlay-{primary_idx}")) {
            let _ = win.set_focus();
        }
    }
    Ok(())
}

/// Topmost frozen window containing the global physical point.
pub fn window_at(session: &CaptureSession, x: i32, y: i32) -> Option<FrozenWindow> {
    session
        .windows
        .iter()
        .find(|w| x >= w.x && x < w.x + w.w && y >= w.y && y < w.y + w.h)
        .cloned()
}

pub fn close_overlays(app: &AppHandle, labels: &[String]) {
    for label in labels {
        if let Some(win) = app.get_webview_window(label) {
            let _ = win.close();
        }
    }
    // Belt and braces: close any stray overlay windows.
    for (label, win) in app.webview_windows() {
        if label.starts_with("overlay-") {
            let _ = win.close();
        }
    }
}

pub fn cancel(app: &AppHandle) -> Result<(), String> {
    let session: &SharedSession = app.state::<SharedSession>().inner();
    let mut guard = session.lock().map_err(|_| "session poisoned")?;
    if let Some(s) = guard.take() {
        close_overlays(app, &s.overlay_labels);
        let _ = std::fs::remove_dir_all(&s.temp_dir);
        restore_main(app, s.main_was_visible);
        let _ = app.emit("capture://cancelled", ());
    }
    Ok(())
}

fn point_in_polygon(px: f64, py: f64, pts: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let n = pts.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > py) != (yj > py) {
            let x_int = (xj - xi) * (py - yi) / (yj - yi) + xi;
            if px < x_int {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Finish a selection: crop the frozen composite (optionally masked by a
/// freeform path), write capture.png, restore the main window and emit
/// `capture://ready`.
pub fn finish(
    app: &AppHandle,
    rect: Option<(i32, i32, i32, i32)>,
    path: Option<Vec<(f64, f64)>>,
    window_id: Option<usize>,
) -> Result<(), String> {
    let session: &SharedSession = app.state::<SharedSession>().inner();
    let result: (Bitmap, PathBuf, i64, (u32, u32), bool) = {
        let mut guard = session.lock().map_err(|_| "session poisoned")?;
        let Some(s) = guard.take() else {
            return Err("No capture session".into());
        };
        let cropped = if let Some(win_id) = window_id {
            let Some(w) = s.windows.iter().find(|w| w.id == win_id) else {
                *guard = Some(s);
                return Err("Selected window no longer exists".into());
            };
            // Window rect in virtual-screen coords -> composite coords.
            let ox = (s.monitors.iter().map(|m| m.x).min().unwrap_or(0)) as i32;
            let oy = (s.monitors.iter().map(|m| m.y).min().unwrap_or(0)) as i32;
            s.composite.crop(w.x - ox, w.y - oy, w.w, w.h)
        } else if let (Some(pts), false) = (path.as_ref(), path.as_ref().map(|p| p.len() < 3).unwrap_or(true)) {
            let xs = pts.iter().map(|p| p.0 as i32);
            let ys = pts.iter().map(|p| p.1 as i32);
            let x0 = xs.clone().min().unwrap_or(0);
            let y0 = ys.clone().min().unwrap_or(0);
            let x1 = xs.max().unwrap_or(0);
            let y1 = ys.max().unwrap_or(0);
            let bw = (x1 - x0).max(2);
            let bh = (y1 - y0).max(2);
            let ox = s.monitors.iter().map(|m| m.x).min().unwrap_or(0);
            let oy = s.monitors.iter().map(|m| m.y).min().unwrap_or(0);
            let mut bmp = s.composite.crop(x0 - ox, y0 - oy, bw, bh);
            // Per-pixel alpha mask with 4-sample supersampled edges.
            let mut mask = vec![0u8; (bw * bh) as usize];
            let ptsf: Vec<(f64, f64)> = pts.iter().map(|p| (p.0 - ox as f64, p.1 - oy as f64)).collect();
            for row in 0..bh as usize {
                for col in 0..bw as usize {
                    let px = col as f64 + x0 as f64 - ox as f64;
                    let py = row as f64 + y0 as f64 - oy as f64;
                    let mut hits = 0;
                    for (ox2, oy2) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                        if point_in_polygon(px + ox2, py + oy2, &ptsf) {
                            hits += 1;
                        }
                    }
                    mask[row * bw as usize + col] = (hits * 63).clamp(0, 255) as u8;
                }
            }
            bmp.apply_alpha_mask(&mask);
            bmp
        } else if let Some((x, y, w, h)) = rect {
            let ox = s.monitors.iter().map(|m| m.x).min().unwrap_or(0);
            let oy = s.monitors.iter().map(|m| m.y).min().unwrap_or(0);
            s.composite.crop(x - ox, y - oy, w, h)
        } else {
            *guard = Some(s);
            return Err("Empty selection".into());
        };
        if cropped.width < 4 || cropped.height < 4 {
            *guard = Some(s);
            return Err("Selection too small".into());
        }
        let png = match cropped.encode_png() {
            Ok(p) => p,
            Err(e) => {
                *guard = Some(s);
                return Err(e.to_string());
            }
        };
        let cap_path = s.temp_dir.join("capture.png");
        if let Err(e) = std::fs::write(&cap_path, &png) {
            *guard = Some(s);
            return Err(format!("Failed to write capture: {e}"));
        }
        let dims = (cropped.width, cropped.height);
        // Keep the temp dir for the editor; prune per-monitor bitmaps.
        for m in &s.monitors {
            let _ = std::fs::remove_file(s.temp_dir.join(format!("monitor-{}.png", m.index)));
        }
        let captured_at = s.captured_at;
        let main_was_visible = s.main_was_visible;
        close_overlays(app, &s.overlay_labels);
        (cropped, cap_path, captured_at, dims, main_was_visible)
    };

    let (bmp, cap_path, captured_at, dims, main_was_visible) = result;
    restore_main(app, main_was_visible);
    let _ = bmp;
    let _ = app.emit(
        "capture://ready",
        serde_json::json!({
            "tempPath": cap_path.to_string_lossy(),
            "fileName": suggested_file_name(captured_at),
            "width": dims.0,
            "height": dims.1,
            "source": "capture",
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggested_filename_shape() {
        let name = suggested_file_name(0);
        assert!(name.starts_with("Snapora_"));
        assert_eq!(name.len(), "Snapora_1970-01-01_00-00-00".len());
        let tail = &name["Snapora_".len()..];
        assert_eq!(tail.as_bytes()[4], b'-');
        assert_eq!(tail.as_bytes()[10], b'_');
    }
}
