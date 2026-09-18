// pxl-asset-bake/tool/render_brand.rs

//! The app's icons and link-preview image, from the brand sources in
//! `asset/brand` into `apps/web/public`, which the deploy ships as static
//! assets.
//!
//! Re-runnable, and the outputs are committed — a changed source is followed
//! by running this and committing what it wrote.
//!
//! Sizes: apple-touch-icon 180, flattened onto the preview's own sky because
//! iOS paints transparency black, and inset a little; the manifest's 192 and
//! 512 `any` icons transparent; a 512 `maskable` icon flattened and inset by
//! the safe zone; an ICO of 16/32/48 as PNG entries, which every browser since
//! Vista reads; and the preview re-encoded as JPEG for the crawlers.
//!
//!   deno task render:brand

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::{ImageFormat, Rgba, RgbaImage};

use pxl_asset_bake::brand;

/// The preview's sky, the ground an opaque icon sits on.
const SKY: Rgba<u8> = Rgba([0x3e, 0xc8, 0xf5, 0xff]);
const CLEAR: Rgba<u8> = Rgba([0, 0, 0, 0]);

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap_or_else(|_| PathBuf::from("."))
}

fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// One icon: the symbol fitted inside `size` less its inset, centred on a
/// field that is either the sky or nothing.
fn icon(symbol: &RgbaImage, size: u32, out: &Path, flatten: bool, pad: f64) -> Result<(), String> {
    let inner = (size as f64 * (1.0 - pad)).round() as u32;
    let canvas = brand::centre_on(&brand::fit(symbol, inner, inner), size, size, if flatten { SKY } else { CLEAR });
    std::fs::create_dir_all(out.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
    std::fs::write(out, png_bytes(&canvas)?).map_err(|e| format!("{}: {e}", out.display()))
}

/// An ICO holding one PNG per size. The directory is 6 bytes then 16 per
/// entry, and a 256-pixel side would be written as 0 — no size here is that
/// large, and the format has no way to say it otherwise.
fn ico(symbol: &RgbaImage, sizes: &[u32], out: &Path) -> Result<(), String> {
    let entries: Vec<Vec<u8>> = sizes
        .iter()
        .map(|s| png_bytes(&brand::centre_on(&brand::fit(symbol, *s, *s), *s, *s, CLEAR)))
        .collect::<Result<_, _>>()?;

    let header_len = 6 + 16 * sizes.len();
    let mut header = vec![0u8; header_len];
    header[2..4].copy_from_slice(&1u16.to_le_bytes());
    header[4..6].copy_from_slice(&(sizes.len() as u16).to_le_bytes());

    let mut offset = header_len as u32;
    for (i, (s, entry)) in sizes.iter().zip(entries.iter()).enumerate() {
        let e = 6 + i * 16;
        header[e] = *s as u8;
        header[e + 1] = *s as u8;
        header[e + 4..e + 6].copy_from_slice(&1u16.to_le_bytes());
        header[e + 6..e + 8].copy_from_slice(&32u16.to_le_bytes());
        header[e + 8..e + 12].copy_from_slice(&(entry.len() as u32).to_le_bytes());
        header[e + 12..e + 16].copy_from_slice(&offset.to_le_bytes());
        offset += entry.len() as u32;
    }

    let mut file = header;
    for entry in entries {
        file.extend_from_slice(&entry);
    }
    std::fs::write(out, file).map_err(|e| format!("{}: {e}", out.display()))
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
    let root = root();
    let out = root.join("apps/web/public");
    // The preview is kept as PNG: it was delivered as a 10-bit AVIF, which
    // sharp's libheif did not decode.
    let preview = image::open(root.join("asset/brand/og.png")).map_err(|e| format!("og.png: {e}"))?.to_rgba8();
    let symbol = brand::trim(&image::open(root.join("asset/brand/symbol.png")).map_err(|e| format!("symbol.png: {e}"))?.to_rgba8(), 8);

    icon(&symbol, 180, &out.join("favicon/apple-touch-icon.png"), true, 0.16)?;
    icon(&symbol, 192, &out.join("favicon/web-app-manifest-192x192.png"), false, 0.06)?;
    icon(&symbol, 512, &out.join("favicon/web-app-manifest-512x512.png"), false, 0.06)?;
    icon(&symbol, 512, &out.join("favicon/web-app-manifest-512x512-maskable.png"), true, 0.24)?;
    ico(&symbol, &[16, 32, 48], &out.join("favicon/favicon.ico"))?;

    // mozjpeg rather than a plain baseline encoder: the same picture at
    // quality 88 is about 30 kB through this and about 55 kB without it, and
    // the preview is what every crawler unfurling a link fetches.
    let rgb = image::DynamicImage::ImageRgba8(preview).to_rgb8();
    let mut enc = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    enc.set_size(rgb.width() as usize, rgb.height() as usize);
    enc.set_quality(88.0);
    let mut started = enc.start_compress(Vec::new()).map_err(|e| e.to_string())?;
    started.write_scanlines(rgb.as_raw()).map_err(|e| e.to_string())?;
    let encoded = started.finish().map_err(|e| e.to_string())?;
    let jpeg = out.join("og-image.jpg");
    std::fs::write(&jpeg, encoded).map_err(|e| format!("{}: {e}", jpeg.display()))?;

    println!("[brand] icons and og-image written to apps/web/public");
    Ok(())
}
