// Windows clipboard image copying via arboard (CF_DIB — pastes into Word,
// Paint, Discord, Teams, email clients, etc.).

use base64::Engine;
use image::ImageDecoder;

pub fn decode_b64_png(data_b64: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(data_b64.trim())
        .map_err(|e| format!("Invalid image data: {e}"))?;
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(&raw))
        .map_err(|e| format!("Invalid PNG data: {e}"))?;
    let (w, h) = decoder.dimensions();
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    decoder
        .read_image(&mut rgba)
        .map_err(|e| format!("PNG decode failed: {e}"))?;
    Ok((w, h, rgba))
}

/// Copy an RGBA image to the clipboard. Discord/Word/Teams paste requires the
/// bitmap to survive after the app keeps running in the tray — Windows
/// clipboard data set via SetClipboardData persists independently of the
/// setting process, so this is safe.
pub fn copy_image_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Clipboard unavailable: {e}"))?;
    let data = arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: std::borrow::Cow::from(rgba),
    };
    cb.set_image(data)
        .map_err(|e| format!("Clipboard copy failed: {e}"))?;
    Ok(())
}

/// Copy a base64-encoded PNG to the clipboard.
pub fn copy_image_png_b64(data_b64: &str) -> Result<(), String> {
    let (w, h, rgba) = decode_b64_png(data_b64)?;
    copy_image_rgba(w, h, rgba)
}
