// Screenshot storage: default directory, collision-safe filenames, encoding
// to PNG / JPEG / WebP.

use crate::error::AppResult;
use crate::settings::ImageFormat;
use base64::Engine;
use image::ImageEncoder;
use std::path::{Path, PathBuf};

pub fn default_save_dir() -> PathBuf {
    // %USERPROFILE%\Pictures\Snapora (Pictures resolves via known folders).
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Pictures")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Snapora")
}

pub fn save_dir(settings: &crate::settings::Settings) -> PathBuf {
    settings
        .save_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(default_save_dir)
}

/// Build a non-colliding path: `stem.ext`, `stem_1.ext`, `stem_2.ext`, …
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    if !candidate.exists() {
        return candidate;
    }
    let mut n = 1;
    loop {
        candidate = dir.join(format!("{stem}_{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

#[allow(dead_code)] // direct RGBA encoder retained as an alternative to transcoding
pub fn encode_image_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
    format: ImageFormat,
) -> AppResult<Vec<u8>> {
    match format {
        ImageFormat::Png => {
            let mut out = Vec::with_capacity(rgba.len() / 4);
            let enc = image::codecs::png::PngEncoder::new(&mut out);
            enc.write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
                .map_err(|e| format!("PNG encode failed: {e}"))?;
            Ok(out)
        }
        ImageFormat::Jpeg => {
            // The JPEG encoder accepts RGBA8 and drops the alpha channel itself.
            let mut out = Vec::with_capacity(rgba.len() / 8);
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
            enc.write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
            Ok(out)
        }
        ImageFormat::WebP => {
            let mut out = Vec::with_capacity(rgba.len() / 8);
            let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
            enc.write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
                .map_err(|e| format!("WebP encode failed: {e}"))?;
            Ok(out)
        }
    }
}

/// Decode a base64 PNG and re-encode into the requested format.
pub fn transcode_b64_png(data_b64: &str, format: ImageFormat) -> AppResult<Vec<u8>> {
    crate::security::check_b64_size(data_b64)?;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(data_b64.trim())
        .map_err(|e| format!("Invalid image data: {e}"))?;
    let img = image::load_from_memory_with_format(&raw, image::ImageFormat::Png)
        .map_err(|e| format!("Invalid PNG data: {e}"))?;
    crate::security::check_dimensions(img.width(), img.height())?;
    match format {
        ImageFormat::Png => Ok(raw),
        ImageFormat::Jpeg => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = Vec::with_capacity(rgba_len(w, h) / 8);
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
            enc.write_image(rgb.as_raw(), w, h, image::ExtendedColorType::Rgb8)
                .map_err(|e| format!("JPEG encode failed: {e}"))?;
            Ok(out)
        }
        ImageFormat::WebP => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let mut out = Vec::with_capacity(rgba_len(w, h) / 8);
            let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
            enc.write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
                .map_err(|e| format!("WebP encode failed: {e}"))?;
            Ok(out)
        }
    }
}

fn rgba_len(w: u32, h: u32) -> usize {
    (w as usize) * (h as usize) * 4
}

/// Write encoded bytes to a path, creating parent dirs; returns the final path.
pub fn write_image_file(path: &Path, bytes: &[u8]) -> AppResult<PathBuf> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(path.to_path_buf())
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedInfo {
    pub path: String,
    pub file_name: String,
    pub dir: String,
}

pub fn save_bytes(
    dir: &Path,
    file_stem: &str,
    format: ImageFormat,
    bytes: &[u8],
) -> AppResult<SavedInfo> {
    let path = unique_path(dir, file_stem, format.extension());
    let saved = write_image_file(&path, bytes)?;
    Ok(SavedInfo {
        path: saved.to_string_lossy().to_string(),
        file_name: saved
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        dir: saved
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    })
}

pub fn extension_format(path: &Path) -> Option<ImageFormat> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Some(ImageFormat::Png),
        Some("jpg") | Some("jpeg") => Some(ImageFormat::Jpeg),
        Some("webp") => Some(ImageFormat::WebP),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_path_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("snapora-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let first = unique_path(&dir, "shot", "png");
        assert_eq!(first.file_name().unwrap(), "shot.png");
        std::fs::write(&first, b"x").unwrap();
        let second = unique_path(&dir, "shot", "png");
        assert_eq!(second.file_name().unwrap(), "shot_1.png");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn extension_format_mapping() {
        assert_eq!(extension_format(Path::new("a.PNG")), Some(ImageFormat::Png));
        assert_eq!(extension_format(Path::new("a.jpeg")), Some(ImageFormat::Jpeg));
        assert_eq!(extension_format(Path::new("a.webp")), Some(ImageFormat::WebP));
        assert_eq!(extension_format(Path::new("a.gif")), None);
    }
}

#[cfg(test)]
mod jpeg_tests {
    use super::*;
    use base64::Engine;
    use image::ImageDecoder;
    use image::ImageEncoder;

    #[test]
    fn jpeg_transcode_preserves_color() {
        let mut rgba = vec![0u8; 8 * 8 * 4];
        for px in rgba.chunks_exact_mut(4) {
            px[0] = 220; px[1] = 40; px[2] = 30; px[3] = 255;
        }
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&rgba, 8, 8, image::ExtendedColorType::Rgba8)
            .unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let jpg = transcode_b64_png(&b64, ImageFormat::Jpeg).unwrap();
        assert_eq!(&jpg[0..2], &[0xFF, 0xD8]); // JPEG SOI marker
        let dec = image::codecs::jpeg::JpegDecoder::new(std::io::Cursor::new(&jpg)).unwrap();
        let (w, h) = dec.dimensions();
        assert_eq!((w, h), (8, 8));
        let mut out = vec![0u8; (w * h * 3) as usize];
        dec.read_image(&mut out).unwrap();
        assert!(out[0] > 150, "red channel should survive: {}", out[0]);
        assert!(out[2] < 110, "blue channel should stay low: {}", out[2]);
    }
}
