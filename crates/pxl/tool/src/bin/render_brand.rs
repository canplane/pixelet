// pxl-tool/bin/render_brand.rs

//! The app's icons and link-preview image, from the three sources in
//! `apps/web/brand` into `apps/web/public`, which the deploy ships as static assets.
//!
//! - `symbol.png` — the cube on a transparent field, for every place that
//!   draws the icon's own shape: the favicon and the manifest's `any` icons.
//! - `icon.png` — the cube's face, edge to edge and opaque, for every place
//!   that cuts the icon to a frame of its own: iOS's rounded square and
//!   Android's maskable shapes. It has no subject to keep inside a safe zone,
//!   so a platform's crop loses nothing and it is drawn with no inset; and it
//!   has no alpha, which both platforms would otherwise paint black.
//! - `og.png` — the link preview, 1200×630. Kept as PNG: it was delivered as a
//!   10-bit AVIF under a `.png` name, which not every decoder reads.
//!
//! Re-runnable; the outputs are committed, so a changed source is followed by
//! running this and committing what it wrote.
//!
//!   deno task render:brand

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageFormat, Rgba, RgbaImage};

use pxl_tool::root;

const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);

fn brand() -> PathBuf {
    root().join("apps/web/brand")
}

fn open(name: &str) -> Result<RgbaImage, String> {
    Ok(image::open(brand().join(name)).map_err(|e| format!("{name}: {e}"))?.to_rgba8())
}

fn write(out: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(out.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
    std::fs::write(out, bytes).map_err(|e| format!("{}: {e}", out.display()))
}

fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Without the alpha channel an opaque icon carries for nothing: the maskable
/// icon is a fifth smaller written as RGB.
fn opaque_png_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img.clone()).to_rgb8().write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Lanczos3, because the sources are painted well above the sizes they are
/// drawn at and a box filter would alias the detail they were chosen for.
fn resize(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    image::imageops::resize(img, w, h, FilterType::Lanczos3)
}

/// The bounding box of everything that differs from the corner pixel by more
/// than `threshold`, per channel.
///
/// The symbol's margin is not the same on every side, and it carries a soft
/// glow in its alpha. Keeping the margin makes a size mean the image rather
/// than the cube, and a 16-pixel favicon then drew a 12-pixel cube.
fn trim(img: &RgbaImage, threshold: u8) -> RgbaImage {
    let bg = *img.get_pixel(0, 0);
    let differs = |p: &Rgba<u8>| p.0.iter().zip(bg.0.iter()).any(|(a, b)| a.abs_diff(*b) > threshold);

    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if differs(p) {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x0 == u32::MAX {
        return img.clone();
    }
    image::imageops::crop_imm(img, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

/// The trimmed symbol fitted inside `size` less its inset, centred on a clear
/// square. One rounding, not two: fitting to the inset and centring on the
/// full square in one step, where two centrings land a pixel off.
fn symbol_icon(symbol: &RgbaImage, size: u32, pad: f64) -> RgbaImage {
    let inner = size as f64 * (1.0 - pad);
    let scale = (inner / symbol.width() as f64).min(inner / symbol.height() as f64);
    let (w, h) = (((symbol.width() as f64 * scale).round() as u32).max(1), ((symbol.height() as f64 * scale).round() as u32).max(1));
    let mut out = RgbaImage::from_pixel(size, size, CLEAR);
    image::imageops::overlay(&mut out, &resize(symbol, w, h), ((size - w) / 2) as i64, ((size - h) / 2) as i64);
    out
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[brand] {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let out = root().join("apps/web/public");
    let symbol = trim(&open("symbol.png")?, 8);
    let icon = open("icon.png")?;
    let preview = open("og.png")?;

    write(&out.join("favicon/apple-touch-icon.png"), &opaque_png_bytes(&resize(&icon, 180, 180))?)?;
    write(&out.join("favicon/web-app-manifest-512x512-maskable.png"), &opaque_png_bytes(&resize(&icon, 512, 512))?)?;
    write(&out.join("favicon/web-app-manifest-192x192.png"), &png_bytes(&symbol_icon(&symbol, 192, 0.06))?)?;
    write(&out.join("favicon/web-app-manifest-512x512.png"), &png_bytes(&symbol_icon(&symbol, 512, 0.06))?)?;

    // One PNG entry per size, which every browser since Vista reads. No inset:
    // at 16 pixels every one of them is the cube's.
    let frames = [16, 32, 48]
        .iter()
        .map(|&s| IcoFrame::as_png(symbol_icon(&symbol, s, 0.0).as_raw(), s, s, ExtendedColorType::Rgba8).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut ico = Vec::new();
    IcoEncoder::new(&mut ico).encode_images(&frames).map_err(|e| e.to_string())?;
    write(&out.join("favicon/favicon.ico"), &ico)?;

    let rgb = DynamicImage::ImageRgba8(preview).to_rgb8();
    let mut enc = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    enc.set_size(rgb.width() as usize, rgb.height() as usize);
    enc.set_quality(88.0);
    let mut started = enc.start_compress(Vec::new()).map_err(|e| e.to_string())?;
    started.write_scanlines(rgb.as_raw()).map_err(|e| e.to_string())?;
    write(&out.join("og-image.jpg"), &started.finish().map_err(|e| e.to_string())?)?;

    println!("[brand] icons and og-image written to {}", out.display());
    Ok(())
}
