# Changelog

All notable changes to Snapora are documented here. This project follows
[semantic versioning](https://semver.org/).

## [1.0.2] — 2026-11-20

### Fixed
- **Print Screen ownership:** Snapora now disables Windows' "Print screen key opens Snipping Tool"
  shortcut by default (with a "Take over Print Screen" toggle in Settings → Keyboard) and
  re-registers its global hotkeys, so pressing Print Screen while minimized launches Snapora rather
  than the built-in screenshot tool.
- **Editor opens automatically after shortcut captures:** the window is now revealed by the frontend
  only after it has switched to the editor view, so a capture taken while minimized opens the editor
  directly instead of the Home screen or nothing at all.

### Changed
- Version bumped to 1.0.2.

## [1.0.1] — 2026-11-20

### Fixed
- **Taskbar overlap:** the main window is now clamped and centred inside the monitor work area at
  startup, so the bottom of the UI is never hidden behind the Windows taskbar (also helps on short
  displays and scaled screens).
- **Shortcut captures no longer raise the main window.** Print Screen captures now restore the window
  to whatever visibility it had before the capture — capturing from the tray stays in the tray
  instead of popping open the Home screen.
- Refreshed documentation screenshots to show the corrected window geometry.

### Changed
- Version bumped to 1.0.1 across the Rust crate, Tauri config, npm package and UI.

## [1.0.0] — 2026-11-20

First public release.

### Added
- Seven capture modes: region, freeform lasso, window pick, active window, fullscreen, single monitor
  and all monitors, with multi-monitor/DPI-aware coordinates.
- Global Print Screen shortcuts (`PrtSc`, `Alt+PrtSc`, `Ctrl+PrtSc`, `Shift+PrtSc`) with conflict
  detection and a live registration-status panel.
- Full canvas editor: crop, resize, rotate, flip, pen, text, arrows, lines, rectangles, ellipses,
  highlighter, blur, pixelation, undo, redo and reset.
- Clipboard image export compatible with Word, Paint, Discord, Teams and email clients.
- PNG/JPEG/WebP saving with collision-safe, date-stamped filenames and a configurable save directory.
- History with thumbnails, metadata, copy/delete/reveal actions.
- System tray background mode, quick captures, autostart and close-to-tray.
- Light/dark/system themes and persisted settings.
- Standalone EXE and x64 MSI installer with Start Menu and desktop shortcuts.
- Documentation: README with screenshots, build guide and architecture notes.

### Fixed
- Undo/redo now restores canvas dimensions after crop, rotate and resize.
- JPEG encoding keeps full colour; WebP uses the correct wire value in settings.

[1.0.2]: https://github.com/Kaziopu5938/Snapora/releases/tag/v1.0.2
[1.0.1]: https://github.com/Kaziopu5938/Snapora/releases/tag/v1.0.1
[1.0.0]: https://github.com/Kaziopu5938/Snapora/releases/tag/v1.0.0
