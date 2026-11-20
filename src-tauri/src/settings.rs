// Application settings: persisted JSON in the app config dir.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFormat {
    Png,
    Jpeg,
    #[serde(rename = "webp")] // keep the wire value lowercase for the frontend
    WebP,
}

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::WebP => "webp",
        }
    }

    #[allow(dead_code)] // MIME type kept for future upload/share features
    pub fn mime(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::WebP => "image/webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    Region,
    Freeform,
    WindowSelect,
    ActiveWindow,
    Fullscreen,
    AllMonitors,
    Monitor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HotkeySettings {
    pub region_enabled: bool,
    pub active_window_enabled: bool,
    pub fullscreen_enabled: bool,
    pub freeform_enabled: bool,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            region_enabled: true,
            active_window_enabled: true,
            fullscreen_enabled: true,
            freeform_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    // General
    pub start_with_windows: bool,
    pub minimize_to_tray: bool,
    pub launch_minimized: bool,
    pub save_dir: Option<String>,
    pub format: ImageFormat,
    // Capture
    pub default_mode: CaptureMode,
    pub delay_secs: u32,
    pub include_cursor: bool,
    pub fullscreen_all_monitors: bool,
    // Keyboard
    pub hotkeys: HotkeySettings,
    /// Disable Windows' "Print screen opens Snipping Tool" shortcut so our
    /// global hotkeys actually receive the key (Windows 10/11).
    #[serde(default = "default_true")]
    pub take_over_print_screen: bool,
    // Appearance
    pub theme: Theme,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            start_with_windows: false,
            minimize_to_tray: true,
            launch_minimized: false,
            save_dir: None,
            format: ImageFormat::Png,
            default_mode: CaptureMode::Region,
            delay_secs: 0,
            include_cursor: false,
            fullscreen_all_monitors: true,
            hotkeys: HotkeySettings::default(),
            take_over_print_screen: true,
            theme: Theme::System,
        }
    }
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .expect("failed to resolve app config dir");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("settings.json")
}

pub fn load_or_default(app: &AppHandle) -> Settings {
    let path = config_path(app);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn persist(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = config_path(app);
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    // Atomic-ish write: temp file + rename so a crash never leaves a torn file.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn image_format_wire_values_are_lowercase() {
        assert_eq!(serde_json::to_string(&ImageFormat::Png).unwrap(), "\"png\"");
        assert_eq!(serde_json::to_string(&ImageFormat::Jpeg).unwrap(), "\"jpeg\"");
        assert_eq!(serde_json::to_string(&ImageFormat::WebP).unwrap(), "\"webp\"");
    }
}
