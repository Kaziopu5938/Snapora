// Top-level window enumeration and window capture (PrintWindow with a
// BitBlt fallback for hardware-rendered windows that print black).

use super::gdi::{capture_rect, window_extended_bounds, window_is_cloaked, Bitmap, PW_RENDERFULLCONTENT};
use crate::error::AppResult;
use std::mem;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
    SelectObject, DIB_RGB_COLORS, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
};
use windows_sys::Win32::Storage::Xps::PrintWindow;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DrawIconEx, EnumWindows, GetCursorInfo, GetForegroundWindow, GetIconInfo, GetSystemMetrics,
    GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, GetWindowRect, IsIconic, IsWindowVisible,
    CURSORINFO, CURSOR_SHOWING, DI_NORMAL, GWL_EXSTYLE, SM_CXCURSOR, SM_CYCURSOR,
    WS_EX_TOOLWINDOW,
};

#[derive(Debug, Clone)]
pub struct FrozenWindow {
    /// Index into the frozen list; given to the overlay UI and resolved back
    /// to an HWND inside Rust only.
    pub id: usize,
    #[allow(dead_code)] // kept for window-activation parity; id is the public handle
    pub hwnd: isize,
    pub title: String,
    /// Visible bounds (DWM extended frame bounds), physical px.
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

fn window_title(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len + 1) as usize];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), (len + 1) as i32);
        if n <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

/// Z-ordered (top first) list of visible, real top-level windows.
pub fn snapshot_windows() -> Vec<FrozenWindow> {
    struct Ctx {
        out: Vec<isize>,
    }
    unsafe extern "system" fn callback(
        hwnd: HWND,
        lparam: windows_sys::Win32::Foundation::LPARAM,
    ) -> i32 {
        let ctx = &mut *(lparam as *mut Ctx);
        unsafe {
            if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 {
                return 1;
            }
            if window_is_cloaked(hwnd) {
                return 1;
            }
            let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
            if (ex as u32) & WS_EX_TOOLWINDOW != 0 {
                return 1;
            }
            let mut r: RECT = mem::zeroed();
            if GetWindowRect(hwnd, &mut r) == 0 {
                return 1;
            }
            if r.right - r.left < 8 || r.bottom - r.top < 8 {
                return 1;
            }
            ctx.out.push(hwnd as isize);
        }
        1
    }
    let mut ctx = Ctx { out: Vec::new() };
    unsafe {
        EnumWindows(
            Some(callback),
            &mut ctx as *mut Ctx as windows_sys::Win32::Foundation::LPARAM,
        );
    }
    let mut out = Vec::new();
    for hwnd in ctx.out {
        let target: HWND = hwnd as HWND;
        // Prefer the DWM visible bounds (excludes invisible resize borders).
        let rect = window_extended_bounds(target).unwrap_or_else(|| {
            let mut r = unsafe { mem::zeroed::<RECT>() };
            unsafe {
                GetWindowRect(target, &mut r);
            }
            r
        });
        let w = rect.right - rect.left;
        let h = rect.bottom - rect.top;
        if w < 8 || h < 8 {
            continue;
        }
        let id = out.len();
        let title = window_title(target);
        out.push(FrozenWindow {
            id,
            hwnd,
            title: if title.is_empty() { "Unnamed window".into() } else { title },
            x: rect.left,
            y: rect.top,
            w,
            h,
        });
    }
    out
}

pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

/// RAII 32bpp top-down DIB section for window capture.
struct WindowDib {
    hdc_mem: windows_sys::Win32::Graphics::Gdi::HDC,
    hbmp: windows_sys::Win32::Graphics::Gdi::HBITMAP,
    old: windows_sys::Win32::Graphics::Gdi::HGDIOBJ,
    bits: *mut u8,
    width: usize,
    height: usize,
    screen_dc: windows_sys::Win32::Graphics::Gdi::HDC,
}

impl WindowDib {
    fn new(width: i32, height: i32) -> AppResult<Self> {
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
            Ok(Self {
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

    fn to_bitmap_opaque(&self) -> Bitmap {
        let len = self.width * self.height * 4;
        let mut rgba = vec![0u8; len];
        if !self.bits.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(self.bits, rgba.as_mut_ptr(), len) };
        }
        for px in rgba.chunks_exact_mut(4) {
            px.swap(0, 2);
            if px[3] == 0 {
                px[3] = 255;
            }
        }
        Bitmap {
            width: self.width as u32,
            height: self.height as u32,
            rgba,
        }
    }

    /// True when the capture looks blank (a known PrintWindow failure mode
    /// with some GPU-accelerated windows).
    fn looks_blank(&self) -> bool {
        if self.bits.is_null() {
            return true;
        }
        let len = self.width * self.height * 4;
        let step = ((len / 4096).max(4)) & !3;
        unsafe {
            let mut i = 0;
            while i + 3 < len {
                if *self.bits.add(i) != 0 || *self.bits.add(i + 1) != 0 {
                    return false;
                }
                i += step;
            }
        }
        true
    }
}

impl Drop for WindowDib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.hdc_mem, self.old);
            DeleteObject(self.hbmp);
            DeleteDC(self.hdc_mem);
            ReleaseDC(std::ptr::null_mut(), self.screen_dc);
        }
    }
}

/// Capture a window's rendered content. Uses PrintWindow(PW_RENDERFULLCONTENT)
/// and falls back to a BitBlt of the window rect when the result is blank.
/// The result is cropped to the DWM visible bounds. `cursor` composites the
/// mouse cursor when it hovers inside the visible window area.
pub fn capture_window(hwnd: isize, cursor: bool) -> AppResult<Bitmap> {
    unsafe {
        let mut wr: RECT = mem::zeroed();
        if GetWindowRect(hwnd as HWND, &mut wr) == 0 {
            return Err("GetWindowRect failed for target window".into());
        }
        let w = wr.right - wr.left;
        let h = wr.bottom - wr.top;
        if w <= 0 || h <= 0 {
            return Err("target window has zero size".into());
        }

        // The crop (DWM visible bounds) relative to the GetWindowRect origin.
        let bounds = window_extended_bounds(hwnd as HWND);
        let (crop_x, crop_y, crop_w, crop_h) = match bounds {
            Some(b) => {
                let cx = (b.left - wr.left).clamp(0, w);
                let cy = (b.top - wr.top).clamp(0, h);
                let cw = (b.right - b.left).clamp(0, w - cx);
                let ch = (b.bottom - b.top).clamp(0, h - cy);
                (cx, cy, cw, ch)
            }
            None => (0, 0, w, h),
        };
        if crop_w <= 0 || crop_h <= 0 {
            return Err("target window is not visible".into());
        }

        let dib = WindowDib::new(w, h)?;
        let printed = PrintWindow(hwnd as HWND, dib.hdc_mem, PW_RENDERFULLCONTENT);
        if printed == 0 || dib.looks_blank() {
            // Fallback: grab whatever is visible on screen at the window rect.
            drop(dib);
            let bmp = capture_rect(wr.left, wr.top, w, h, false)?;
            return Ok(bmp.crop(crop_x, crop_y, crop_w, crop_h));
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
                    let hot_x = info.xHotspot as i32;
                    let hot_y = info.yHotspot as i32;
                    if !info.hbmMask.is_null() {
                        DeleteObject(info.hbmMask);
                    }
                    if !info.hbmColor.is_null() {
                        DeleteObject(info.hbmColor);
                    }
                    // Position relative to the DC origin, so that after the
                    // crop below it lands at the right spot in the bitmap.
                    let dx = ci.ptScreenPos.x - hot_x - wr.left;
                    let dy = ci.ptScreenPos.y - hot_y - wr.top;
                    let cw = GetSystemMetrics(SM_CXCURSOR);
                    let ch = GetSystemMetrics(SM_CYCURSOR);
                    if dx + cw > crop_x && dy + ch > crop_y && dx < crop_x + crop_w && dy < crop_y + crop_h {
                        let _ = DrawIconEx(
                            dib.hdc_mem, dx, dy, ci.hCursor, 0, 0, 0, std::ptr::null_mut(), DI_NORMAL,
                        );
                    }
                }
            }
        }

        let full = dib.to_bitmap_opaque();
        Ok(full.crop(crop_x, crop_y, crop_w, crop_h))
    }
}
