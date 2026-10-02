// pxl-tool/asset/sheet.rs

//! The texture pack as one lossy WebP (q92): about 400 KB against several MB
//! lossless, downloaded on every cold start.

use image::RgbaImage;

pub const CELL: u32 = 256;
pub const COLUMNS: u32 = 8;
const QUALITY: f32 = 92.0;

pub struct Sheet {
    pub bytes: Vec<u8>,
    pub tiles: Vec<String>,
    /// Each tile's mean color, averaged in linear light; a mean of sRGB bytes
    /// reads darker than the tile.
    pub means: Vec<[u8; 3]>,
}

fn mean_of(img: &RgbaImage) -> [u8; 3] {
    let mut sum = [0.0f64; 3];
    for px in img.pixels() {
        for (s, c) in sum.iter_mut().zip(px.0) {
            *s += srgb_to_linear(c);
        }
    }
    let n = (img.width() * img.height()) as f64;
    core::array::from_fn(|k| linear_to_srgb(sum[k] / n))
}

fn srgb_to_linear(v: u8) -> f64 {
    let c = v as f64 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f64) -> u8 {
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Every tile must be exactly one cell; a wrong size is an upstream mistake
/// and is refused rather than resized.
pub fn compose(root: &std::path::Path, tiles: &[String]) -> Result<Sheet, String> {
    let rows = tiles.len().div_ceil(COLUMNS as usize) as u32;
    // Opaque black, so an uncovered cell is visible rather than blended.
    let mut sheet = RgbaImage::from_pixel(COLUMNS * CELL, rows.max(1) * CELL, image::Rgba([0, 0, 0, 255]));
    let mut means = Vec::with_capacity(tiles.len());

    for (n, tile) in tiles.iter().enumerate() {
        let path = root.join(tile);
        let img = image::open(&path).map_err(|e| format!("{tile}: {e}"))?.to_rgba8();
        if img.width() != CELL || img.height() != CELL {
            return Err(format!("{tile} is {}x{}, not {CELL} square", img.width(), img.height()));
        }
        let (x, y) = ((n as u32 % COLUMNS) * CELL, (n as u32 / COLUMNS) * CELL);
        means.push(mean_of(&img));
        image::imageops::replace(&mut sheet, &img, x as i64, y as i64);
    }

    let encoded = webp::Encoder::from_rgba(sheet.as_raw(), sheet.width(), sheet.height()).encode(QUALITY);
    Ok(Sheet {
        bytes: encoded.to_vec(),
        tiles: tiles.to_vec(),
        means,
    })
}
