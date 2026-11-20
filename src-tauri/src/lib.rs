// Snapora — Capture. Edit. Share.
// Application entry: plugins, state, tray, global shortcuts, window events.

mod capture;
mod clipboard;
mod commands;
mod error;
mod history;
mod settings;
mod shortcuts;
mod storage;

use capture::overlay::{CaptureSession, SharedSession};
use settings::{Settings, Theme};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, RunEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt as _};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, ShortcutState};
use tauri_plugin_opener::OpenerExt;

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
    let _ = app.emit("tray://open", ());
}

/// Keep the main window fully inside the monitor's work area so the taskbar
/// never covers the bottom of the UI (important on short displays / high DPI).
fn fit_main_window_to_work_area(app: &AppHandle) {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETWORKAREA};

    let Some(win) = app.get_webview_window("main") else { return };
    let mut area: RECT = unsafe { std::mem::zeroed() };
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut area as *mut RECT as *mut _, 0) } == 0 {
        return;
    }
    let avail_w = (area.right - area.left).max(0) as u32;
    let avail_h = (area.bottom - area.top).max(0) as u32;
    if avail_w < 320 || avail_h < 240 {
        return;
    }

    // Clamp the OUTER size: on Windows the frame adds ~39 px that `set_size`
    // does not account for, which would push the bottom behind the taskbar.
    let inner = win.inner_size().unwrap_or(PhysicalSize::new(1060, 700));
    let outer = win.outer_size().unwrap_or(inner);
    let frame_w = outer.width.saturating_sub(inner.width);
    let frame_h = outer.height.saturating_sub(inner.height);
    let max_outer_w = avail_w.saturating_sub(16);
    let max_outer_h = avail_h.saturating_sub(16);
    let target_outer_w = outer.width.min(max_outer_w);
    let target_outer_h = outer.height.min(max_outer_h);
    let target_inner_w = target_outer_w.saturating_sub(frame_w).max(320);
    let target_inner_h = target_outer_h.saturating_sub(frame_h).max(240);
    let _ = win.set_size(PhysicalSize::new(target_inner_w, target_inner_h));

    // Centre using the real outer size after the resize.
    let final_outer = win.outer_size().unwrap_or(PhysicalSize::new(target_outer_w, target_outer_h));
    let x = area.left + (avail_w.saturating_sub(final_outer.width) / 2) as i32;
    let y = area.top + (avail_h.saturating_sub(final_outer.height) / 2) as i32;
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

fn apply_theme_at_startup(app: &AppHandle, s: &Settings) {
    if let Some(win) = app.get_webview_window("main") {
        let theme = match s.theme {
            Theme::Light => Some(tauri::Theme::Light),
            Theme::Dark => Some(tauri::Theme::Dark),
            Theme::System => None,
        };
        let _ = win.set_theme(theme);
        if s.launch_minimized {
            let _ = win.hide();
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton, MouseButtonState};

    let open = MenuItemBuilder::with_id("open", "Open Snapora").build(app)?;
    let cap_region = MenuItemBuilder::with_id("cap-region", "Region capture (Print Screen)").build(app)?;
    let cap_active = MenuItemBuilder::with_id("cap-active", "Active window (Alt+Print Screen)").build(app)?;
    let cap_full = MenuItemBuilder::with_id("cap-full", "Fullscreen (Ctrl+Print Screen)").build(app)?;
    let cap_free = MenuItemBuilder::with_id("cap-free", "Freeform (Shift+Print Screen)").build(app)?;
    let cap_all = MenuItemBuilder::with_id("cap-all", "Capture all monitors").build(app)?;
    let open_folder = MenuItemBuilder::with_id("open-folder", "Open screenshots folder").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Snapora").build(app)?;

    let menu = MenuBuilder::new(app)
        .item(&open)
        .separator()
        .item(&cap_region)
        .item(&cap_active)
        .item(&cap_full)
        .item(&cap_free)
        .item(&cap_all)
        .separator()
        .item(&open_folder)
        .separator()
        .item(&quit)
        .build()?;

    let icon = app
        .default_window_icon()
        .cloned()
        .unwrap_or_else(|| tauri::image::Image::new(&[], 0, 0));

    TrayIconBuilder::with_id("snapora-tray")
        .icon(icon)
        .menu(&menu)
        .tooltip("Snapora — Capture. Edit. Share.")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let mode = match event.id().as_ref() {
                "cap-region" => Some(settings::CaptureMode::Region),
                "cap-active" => Some(settings::CaptureMode::ActiveWindow),
                "cap-full" => Some(settings::CaptureMode::Fullscreen),
                "cap-free" => Some(settings::CaptureMode::Freeform),
                "cap-all" => Some(settings::CaptureMode::AllMonitors),
                _ => None,
            };
            match event.id().as_ref() {
                "open" => show_main(app),
                "open-folder" => {
                    let dir = {
                        let state = app.state::<Mutex<Settings>>();
                        state
                            .lock()
                            .ok()
                            .map(|s| storage::save_dir(&s))
                    };
                    if let Some(dir) = dir {
                        let _ = std::fs::create_dir_all(&dir);
                        let _ = app.opener().open_path(dir.to_string_lossy(), None::<&str>);
                    }
                }
                "quit" => app.exit(0),
                _ => {}
            }
            if let Some(mode) = mode {
                let handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    if let Err(e) = capture::begin_capture(&handle, mode, None) {
                        let _ = handle.emit("capture://error", e);
                    }
                });
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            ShortcutBuilder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        shortcuts::dispatch(app, shortcut);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::start_capture,
            commands::cancel_capture,
            commands::discard_capture,
            commands::overlay_info,
            commands::overlay_window_at,
            commands::overlay_finish,
            commands::get_monitors,
            commands::copy_image,
            commands::read_image_b64,
            commands::get_save_dir,
            commands::choose_save_dir,
            commands::save_image,
            commands::save_image_as,
            commands::list_history,
            commands::get_thumbnail,
            commands::delete_history_item,
            commands::reveal_in_explorer,
            commands::open_path,
            commands::get_settings,
            commands::save_settings,
            commands::get_shortcut_report,
            commands::open_windows_settings,
        ])
        .setup(|app| {
            let handle = app.handle();

            // Settings first: everything else reads them.
            let settings = settings::load_or_default(handle);
            app.manage(Mutex::new(settings.clone()));
            app.manage::<SharedSession>(Arc::new(Mutex::new(None::<CaptureSession>)));
            app.manage(shortcuts::SharedReport::default());

            capture::overlay::cleanup_stale_temp(handle);
            history::ensure_save_dir(&settings);
            fit_main_window_to_work_area(handle);
            apply_theme_at_startup(handle, &settings);
            shortcuts::register_all(handle, &settings);

            // Autostart state follows the persisted setting.
            let autostart = app.autolaunch();
            let _ = if settings.start_with_windows {
                autostart.enable()
            } else {
                autostart.disable()
            };

            build_tray(handle)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let app = window.app_handle();
                    let tray_mode = {
                        let state = app.state::<Mutex<Settings>>();
                        state
                            .lock()
                            .map(|s| s.minimize_to_tray)
                            .unwrap_or(true)
                    };
                    if tray_mode {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                // Drop any in-flight capture session and its temp files.
                if let Some(session) = app.try_state::<SharedSession>() {
                    if let Ok(mut guard) = session.lock() {
                        if let Some(s) = guard.take() {
                            capture::overlay::close_overlays(app, &s.overlay_labels);
                            let _ = std::fs::remove_dir_all(&s.temp_dir);
                        }
                    }
                }
            }
        });
}
