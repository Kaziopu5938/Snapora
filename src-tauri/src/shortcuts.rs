// Global hotkeys (Print Screen family) with per-shortcut registration status
// and Windows "Print screen opens snipping" conflict detection.

use crate::capture::CaptureMode;
use crate::settings::{HotkeySettings, Settings};
use serde::Serialize;
use std::str::FromStr;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    pub id: String,
    pub label: String,
    pub combo: String,
    pub enabled: bool,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutReport {
    pub items: Vec<ShortcutStatus>,
    /// Some(true/false) when Windows' own "Print screen key opens snipping"
    /// setting could be read; None when the value is absent.
    pub os_snipping_enabled: Option<bool>,
}

pub type SharedReport = Mutex<ShortcutReport>;

const fn combo_region() -> &'static str {
    "PrintScreen"
}
const fn combo_active() -> &'static str {
    "Alt+PrintScreen"
}
const fn combo_fullscreen() -> &'static str {
    "Control+PrintScreen"
}
const fn combo_freeform() -> &'static str {
    "Shift+PrintScreen"
}

/// Registry check: HKCU\Control Panel\Keyboard → PrintScreenKeyForSnippingEnabled
pub fn os_snipping_setting() -> Option<bool> {
    use windows_sys::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
    };
    use std::os::windows::ffi::OsStrExt;
    use std::ffi::OsStr;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }
    let subkey = wide("Control Panel\\Keyboard");
    let value = wide("PrintScreenKeyForSnippingEnabled");
    let mut data: u32 = 0;
    let mut size: u32 = 4;
    let hr = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut data as *mut u32 as *mut std::ffi::c_void,
            &mut size,
        )
    };
    if hr == 0 {
        Some(data == 1)
    } else {
        None
    }
}

/// Enable/disable the Windows "Print screen key opens Snipping Tool" shortcut.
/// Snapora turns this off by default so the physical Print Screen key reaches
/// our global hotkeys instead of the built-in screenshot tool.
pub fn set_os_snipping_setting(enabled: bool) -> Result<(), String> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
    };

    fn wide(s: &str) -> Vec<u16> {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    let subkey = wide("Control Panel\\Keyboard");
    let value = wide("PrintScreenKeyForSnippingEnabled");
    let mut key: *mut std::ffi::c_void = std::ptr::null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut key)
    };
    if status != 0 {
        return Err(format!("Could not open keyboard registry key ({status})"));
    }
    let data: u32 = if enabled { 1 } else { 0 };
    let status = unsafe {
        RegSetValueExW(
            key,
            value.as_ptr(),
            0,
            REG_DWORD,
            &data as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        )
    };
    unsafe { RegCloseKey(key) };
    if status != 0 {
        return Err(format!("Could not change Print Screen setting ({status})"));
    }
    // Tell running apps the keyboard settings changed.
    let mut result: usize = 0;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            wide("Keyboard").as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            1500,
            &mut result,
        );
    }
    Ok(())
}

/// (mode, enabled, combo, id, label) rows in stable order.
fn rows(hotkeys: &HotkeySettings) -> [(CaptureMode, bool, &'static str, &'static str, &'static str); 4] {
    [
        (
            CaptureMode::Region,
            hotkeys.region_enabled,
            combo_region(),
            "region",
            "Region capture",
        ),
        (
            CaptureMode::ActiveWindow,
            hotkeys.active_window_enabled,
            combo_active(),
            "activeWindow",
            "Active window capture",
        ),
        (
            CaptureMode::Fullscreen,
            hotkeys.fullscreen_enabled,
            combo_fullscreen(),
            "fullscreen",
            "Fullscreen capture",
        ),
        (
            CaptureMode::Freeform,
            hotkeys.freeform_enabled,
            combo_freeform(),
            "freeform",
            "Freeform capture",
        ),
    ]
}

/// (Re)register all enabled shortcuts. Stores + emits the report.
pub fn register_all(app: &AppHandle, settings: &Settings) -> ShortcutReport {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();

    let mut items = Vec::new();
    for (mode, enabled, combo, id, label) in rows(&settings.hotkeys) {
        if !enabled {
            items.push(ShortcutStatus {
                id: id.into(),
                label: label.into(),
                combo: combo.into(),
                enabled: false,
                ok: true,
                error: None,
            });
            continue;
        }
        match Shortcut::from_str(combo) {
            Ok(sc) => match gs.register(sc) {
                Ok(()) => items.push(ShortcutStatus {
                    id: id.into(),
                    label: label.into(),
                    combo: combo.into(),
                    enabled: true,
                    ok: true,
                    error: None,
                }),
                Err(e) => items.push(ShortcutStatus {
                    id: id.into(),
                    label: label.into(),
                    combo: combo.into(),
                    enabled: true,
                    ok: false,
                    error: Some(e.to_string()),
                }),
            },
            Err(e) => items.push(ShortcutStatus {
                id: id.into(),
                label: label.into(),
                combo: combo.into(),
                enabled: true,
                ok: false,
                error: Some(format!("Invalid shortcut string: {e}")),
            }),
        }
        let _ = mode;
    }

    let report = ShortcutReport {
        items,
        os_snipping_enabled: os_snipping_setting(),
    };
    if let Some(state) = app.try_state::<SharedReport>() {
        if let Ok(mut r) = state.lock() {
            *r = report.clone();
        }
    }
    let _ = app.emit("shortcuts://updated", &report);
    report
}

/// Global shortcut handler: resolve the pressed combo to a capture mode.
pub fn dispatch(app: &AppHandle, shortcut: &Shortcut) {
    let settings_state = app.try_state::<Mutex<Settings>>();
    let Some(state) = settings_state else { return };
    let Ok(settings) = state.lock() else { return };
    let target = rows(&settings.hotkeys)
        .into_iter()
        .filter(|(_, enabled, _, _, _)| *enabled)
        .find(|(_, _, combo, _, _)| {
            Shortcut::from_str(combo).map(|sc| sc == *shortcut).unwrap_or(false)
        })
        .map(|(mode, ..)| mode);
    drop(settings);

    if let Some(mode) = target {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = crate::capture::begin_capture(&handle, mode, None) {
                let _ = handle.emit("capture://error", e);
            }
        });
    }
}
