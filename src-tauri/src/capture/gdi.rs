// GDI screen capture: monitors, BitBlt grabs, cursor overlay, cropping.
//
// The process is Per-Monitor-V2 DPI aware (Tauri's embedded manifest), so all
// coordinates from Win32 (monitor rects, GetWindowRect, cursor pos) are in
// physical pixels of the virtual desktop.

use crate::error::AppResult;
use image::ImageEncoder;
use std::mem;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows_sys::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors,
    GetDC, GetMonitorInfoW, ReleaseDC, SelectObject, CAPTUREBLT, DIB_RGB_COLORS, HDC, HBITMAP,
    HGDIOBJ, HMONITOR, MONITORINFO, SRCCOPY, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DrawIconEx, GetCursorInfo, GetIconInfo, GetSystemMetrics, CURSORINFO, CURSOR_SHOWING,
    DI_NORMAL, MONITORINFOF_PRIMARY, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};

pub const PW_RENDERFULLCONTENT: u32 = 0x0000_0002;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub dpi: u32,
    pub primary: bool,
}

impl MonitorInfo {
    #[allow(dead_code)] // DPI scale helper retained for future overlay math
    pub fn scale(&self) -> f64 {
        (self.dpi as f64 / 96.0).max(1.0)
    }
}

/// A captured raster in top-down RGBA (alpha forced to 255 for screen grabs).
#[derive(Debug, Clone)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Bitmap {
    pub fn crop(&self, x: i32, y: i32, w: i32, h: i32) -> Bitmap {
        let bx = x.clamp(0, self.width as i32);
        let by = y.clamp(0, self.height as i32);
        let bw = w.clamp(0, self.width as i32 - bx) as u32;
        let bh = h.clamp(0, self.height as i32 - by) as u32;
        let stride = self.width as usize * 4;
        let mut rgba = Vec::with_capacity((bw * bh) as usize * 4);
        for row in 0..bh as usize {
            let start = (by as usize + row) * stride + bx as usize * 4;
            rgba.extend_from_slice(&self.rgba[start..start + bw as usize * 4]);
        }
        Bitmap { width: bw, height: bh, rgba }
    }

    pub fn encode_png(&self) -> AppResult<Vec<u8>> {
        let mut out = Vec::with_capacity(self.rgba.len() / 8);
        let enc = image::codecs::png::PngEncoder::new(&mut out);
        enc.write_image(
            &self.rgba,
            self.width,
            self.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG encode failed: {e}"))?;
        Ok(out)
    }

    /// Apply a per-pixel alpha mask (same dimensions), for freeform capture.
    pub fn apply_alpha_mask(&mut self, mask: &[u8]) {
        for (px, m) in self.rgba.chunks_exact_mut(4).zip(mask.iter()) {
            px[3] = *m;
        }
    }
}

/// RAII wrapper around a 32bpp top-down DIB section selected into a mem DC.
struct DibSection {
    hdc_mem: HDC,
    hbmp: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u8,
    width: usize,
    height: usize,
    screen_dc: HDC,
}

impl DibSection {
    fn new(width: i32, height: i32) -> AppResult<DibSection> {
        unsafe {
            let screen_dc = GetDC(std::ptr::null_mut());
            if screen_dc.is_null() {
                return Err("GetDC(NULL) failed".into());
            }
            let hdc_mem = CreateCompatibleDC(screen_dc);
            if hdc_mem.is_null() {
                ReleaseDC(std::ptr::null_mut(), screen_dc);
                return Err("CreateCompatibleDC failed".into());
            }
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    // Negative height => top-down rows, matching our Bitmap layout.
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [mem::zeroed(); 1],
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let hbmp = CreateDIBSection(
                screen_dc,
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if hbmp.is_null() || bits.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(std::ptr::null_mut(), screen_dc);
                return Err("CreateDIBSection failed".into());
            }
            let old = SelectObject(hdc_mem, hbmp);
            Ok(DibSection {
                hdc_mem,
                hbmp,
                old,
                bits: bits as *mut u8,
                width: width as usize,
                height: height as usize,
                screen_dc,
            })
        }
    }

    /// Copy the BGRA DIB rows out and convert them into an RGBA Bitmap.
    fn to_bitmap(&self) -> Bitmap {
        let len = self.width * self.height * 4;
        let mut rgba = vec![0u8; len];
        if !self.bits.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(self.bits, rgba.as_mut_ptr(), len) };
        }
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2); // BGRA -> RGBA
            if px[3] == 0 {
                px[3] = 255; // GDI never writes alpha; screen content is opaque
            }
        }
        Bitmap {
            width: self.width as u32,
            height: self.height as u32,
            rgba,
        }
    }
}

impl Drop for DibSection {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc_mem, self.old);
            DeleteObject(self.hbmp);
            DeleteDC(self.hdc_mem);
            ReleaseDC(std::ptr::null_mut(), self.screen_dc);
        }
    }
}

/// Single-pass monitor enumeration: physical rect, effective DPI, primary flag.
pub fn enumerate_monitors() -> Vec<MonitorInfo> {
    struct Ctx {
        out: Vec<MonitorInfo>,
    }
    unsafe extern "system" fn callback(
        hmon: HMONITOR,
        _hdc: HDC,
        rect: *mut RECT,
        lparam: windows_sys::Win32::Foundation::LPARAM,
    ) -> i32 {
        let ctx = &mut *(lparam as *mut Ctx);
        let r = unsafe { *rect };
        unsafe {
            let mut dpi_x: u32 = 96;
            let mut dpi_y: u32 = 96;
            if GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) != 0 {
                dpi_x = 96;
            }
            let mut mi: MONITORINFO = mem::zeroed();
            mi.cbSize = mem::size_of::<MONITORINFO>() as u32;
            let primary = if GetMonitorInfoW(hmon, &mut mi) != 0 {
                (mi.dwFlags & MONITORINFOF_PRIMARY) != 0
            } else {
                false
            };
            let index = ctx.out.len();
            ctx.out.push(MonitorInfo {
                index,
                x: r.left,
                y: r.top,
                width: r.right - r.left,
                height: r.bottom - r.top,
                dpi: dpi_x,
                primary,
            });
        }
        1
    }
    let mut ctx = Ctx { out: Vec::new() };
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            Some(callback),
            &mut ctx as *mut Ctx as windows_sys::Win32::Foundation::LPARAM,
        );
    }
    let mut monitors = ctx.out;
    if monitors.is_empty() {
        // Fallback: virtual screen bounds from system metrics.
        let (x, y, w, h) = unsafe {
            (
                GetSystemMetrics(SM_XVIRTUALSCREEN),
                GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN),
                GetSystemMetrics(SM_CYVIRTUALSCREEN),
            )
        };
        monitors.push(MonitorInfo {
            index: 0,
            x,
            y,
            width: w,
            height: h,
            dpi: 96,
            primary: true,
        });
    }
    // Stable order: left-to-right, top-to-bottom.
    monitors.sort_by(|a, b| (a.x, a.y).cmp(&(b.x, b.y)));
    for (i, m) in monitors.iter_mut().enumerate() {
        m.index = i;
    }
    monitors
}

/// Union rect of all monitors, in physical pixels.
pub fn virtual_screen_bounds() -> (i32, i32, i32, i32) {
    let monitors = enumerate_monitors();
    let x0 = monitors.iter().map(|m| m.x).min().unwrap_or(0);
    let y0 = monitors.iter().map(|m| m.y).min().unwrap_or(0);
    let x1 = monitors.iter().map(|m| m.x + m.width).max().unwrap_or(0);
    let y1 = monitors.iter().map(|m| m.y + m.height).max().unwrap_or(0);
    (x0, y0, x1 - x0, y1 - y0)
}

/// Capture the rectangle (physical px, virtual-screen coords) from the screen.
/// If `cursor` is true the mouse cursor is composited into the result.
pub fn capture_rect(x: i32, y: i32, w: i32, h: i32, cursor: bool) -> AppResult<Bitmap> {
    if w <= 0 || h <= 0 {
        return Err("capture rect is empty".into());
    }
    unsafe {
        let dib = DibSection::new(w, h)?;
        let screen_dc = GetDC(std::ptr::null_mut());
        if screen_dc.is_null() {
            return Err("GetDC(NULL) failed".into());
        }
        let ok = BitBlt(dib.hdc_mem, 0, 0, w, h, screen_dc, x, y, SRCCOPY | CAPTUREBLT);
        ReleaseDC(std::ptr::null_mut(), screen_dc);
        if ok == 0 {
            return Err("BitBlt failed (screen capture unavailable)".into());
        }
        if cursor {
            let mut ci: CURSORINFO = mem::zeroed();
            ci.cbSize = mem::size_of::<CURSORINFO>() as u32;
            if GetCursorInfo(&mut ci) != 0
                && (ci.flags & CURSOR_SHOWING) != 0
                && !ci.hCursor.is_null()
            {
                let mut info: windows_sys::Win32::UI::WindowsAndMessaging::ICONINFO = mem::zeroed();
                if GetIconInfo(ci.hCursor, &mut info) != 0 {
                    let dx = ci.ptScreenPos.x - info.xHotspot as i32 - x;
                    let dy = ci.ptScreenPos.y - info.yHotspot as i32 - y;
                    let _ = DrawIconEx(
                        dib.hdc_mem,
                        dx,
                        dy,
                        ci.hCursor,
                        0,
                        0,
                        0,
                        std::ptr::null_mut(),
                        DI_NORMAL,
                    );
                    if !info.hbmMask.is_null() {
                        DeleteObject(info.hbmMask);
                    }
                    if !info.hbmColor.is_null() {
                        DeleteObject(info.hbmColor);
                    }
                }
            }
        }
        let bmp = dib.to_bitmap();
        drop(dib);
        Ok(bmp)
    }
}

pub fn capture_virtual_screen(cursor: bool) -> AppResult<Bitmap> {
    let (x, y, w, h) = virtual_screen_bounds();
    capture_rect(x, y, w, h, cursor)
}

/// DWM "extended frame bounds" — the visible part of a window (no invisible
/// resize borders). Falls back to GetWindowRect when DWM refuses.
pub fn window_extended_bounds(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let mut rect: RECT = mem::zeroed();
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut rect as *mut RECT as *mut std::ffi::c_void,
            mem::size_of::<RECT>() as u32,
        );
        if hr == 0 {
            Some(rect)
        } else {
            None
        }
    }
}

/// Returns true when the window is a UWP "cloaked" (invisible) shell window.
pub fn window_is_cloaked(hwnd: HWND) -> bool {
    unsafe {
        let mut cloaked: u32 = 0;
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            &mut cloaked as *mut u32 as *mut std::ffi::c_void,
            mem::size_of::<u32>() as u32,
        );
        hr == 0 && cloaked != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(width: u32, height: u32) -> Bitmap {
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let i = ((y * width + x) * 4) as usize;
                rgba[i] = (x * 30) as u8;
                rgba[i + 1] = (y * 30) as u8;
                rgba[i + 2] = 128;
                rgba[i + 3] = 255;
            }
        }
        Bitmap { width, height, rgba }
    }

    #[test]
    fn crop_and_png_roundtrip() {
        let cropped = sample(8, 8).crop(2, 2, 4, 4);
        assert_eq!((cropped.width, cropped.height), (4, 4));
        assert_eq!(cropped.rgba.len(), 4 * 4 * 4);
        let png = cropped.encode_png().unwrap();
        assert_eq!(&png[0..8], b"\x89PNG\r\n\x1a\n");
        // PNG IHDR stores width/height as big-endian u32 at offsets 16 and 20.
        assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 4);
        assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 4);
    }

    #[test]
    fn alpha_mask_applies_per_pixel() {
        let mut bmp = sample(2, 1);
        bmp.apply_alpha_mask(&[0, 255]);
        assert_eq!(bmp.rgba[3], 0);
        assert_eq!(bmp.rgba[7], 255);
    }

    #[test]
    fn virtual_screen_capture_is_valid() {
        assert!(!enumerate_monitors().is_empty(), "no monitor in this session");
        let bmp = capture_virtual_screen(false).expect("screen capture failed");
        assert!(bmp.width > 0 && bmp.height > 0);
        assert_eq!(bmp.rgba.len(), (bmp.width * bmp.height * 4) as usize);
        assert!(!bmp.encode_png().unwrap().is_empty());
    }
}
