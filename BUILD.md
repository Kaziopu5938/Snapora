# Building Snapora

## Prerequisites

| Component | Notes |
| --- | --- |
| Rust 1.77+ | `x86_64-pc-windows-gnu` or the default MSVC target |
| Node.js 20+ | npm included |
| WebView2 Runtime | Present on Windows 11 / recent Windows 10, or installed by the MSI bootstrapper |
| WiX Toolset 3.x | **Not required manually** — `tauri build` downloads WiX 3.14 automatically when missing |
| MSVC Build Tools | Preferred. Without them, use the GNU fallback below |

### GNU toolchain fallback (used to produce this release)

This host had Rust's `x86_64-pc-windows-gnu` toolchain but no MSVC Build Tools. A portable
[w64devkit](https://github.com/peterthoeny/w64devkit) supplies `gcc`, `windres` and `ld`:

1. Download `w64devkit.exe` and run it; it extracts to a `w64devkit` folder.
2. Because Rust's GNU target links `-lgcc_eh` but w64devkit ships `libgcc.a`, create the alias:
   ```powershell
   $dir = "<w64devkit>\lib\gcc\x86_64-w64-mingw32\<version>"
   Copy-Item "$dir\libgcc.a" "$dir\libgcc_eh.a"
   ```
3. Add `<w64devkit>\bin` to `PATH` before building (the script below does this).

The app's `[lib] crate-type` is deliberately `["rlib"]` (desktop-only). The Tauri default
`cdylib` crate type produces an oversized DLL export table that GNU `ld` rejects; it is not needed
for a Windows desktop application.

## Development

```powershell
npm install
npm run tauri dev
```

`npm run build` type-checks with `tsc --noEmit` and produces `dist/`.
`npm run tauri dev` starts Vite on `localhost:1420` and opens the Tauri window.

## Release build

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

The script:
1. builds the frontend (`npm run build`);
2. forces the embedded frontend to refresh;
3. runs `npm run tauri build` (release Rust binary + MSI);
4. copies the deliverables into `release/`.

Manual equivalent:

```powershell
$env:PATH = "<w64devkit>\bin;$env:PATH"   # only for the GNU fallback
npm run build
npm run tauri build
```

Outputs:

- `src-tauri/target/release/snapora.exe`
- `src-tauri/target/release/WebView2Loader.dll`
- `src-tauri/target/release/bundle/msi/Snapora_1.0.0_x64_en-US.msi`

## Installer notes

- The MSI targets x64, installs per-user without elevation, creates Start Menu and desktop
  shortcuts, and registers a clean uninstall.
- Screenshots (`%USERPROFILE%\Pictures\Snapora`) and app data (settings, thumbnail cache) live
  outside the install directory and are **preserved** on uninstall.
- The desktop shortcut is created by the WiX configuration; remove it manually if undesired.

## Manual verification checklist

Run the installed app and confirm:

1. Each capture mode produces the expected image; region/freeform/window overlays align with the
   frozen desktop on a scaled (125%/150%) display.
2. `PrtSc`, `Alt+PrtSc`, `Ctrl+PrtSc` and `Shift+PrtSc` work from another application and while
   Snapora is in the tray. If a combo shows "Blocked", follow the Windows keyboard-setting guidance
   in Settings → Keyboard.
3. `Ctrl+C` in the editor, then paste into Word, Paint, Discord, Teams and an email client.
4. PNG, JPEG and WebP saves; custom save directory; Save As; collision-safe filenames.
5. Settings persist across a full quit and relaunch; theme follows System when selected.
6. Close-to-tray, tray quick captures, tray quit, and Start with Windows.
7. Install the MSI, launch from the Start Menu, uninstall, and verify screenshots/settings remain.
