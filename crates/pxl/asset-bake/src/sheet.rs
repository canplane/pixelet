// pxl-asset-bake/sheet.rs

//! The texture pack as one image.
//!
//! The client's layer array is filled from a single download rather than from
//! sixty: the pack is one file, which is what a pack is. Tile `n` sits at
//! column `n % COLUMNS`, row `n / COLUMNS`, and the order is the pack's own
//! path order — which is what the generated table records, so nothing has to
//! agree about it twice.
//!
//! Lossy WebP at quality 92, through libwebp. Sixty 256² tiles lossless is
//! several megabytes; this is around four hundred kilobytes, and it is
//! downloaded on every cold start.

use image::RgbaImage;

pub const CELL: u32 = 256;
pub const COLUMNS: u32 = 8;
const QUALITY: f32 = 92.0;

pub struct Sheet {
    pub bytes: Vec<u8>,
    /// The asset path of each tile, in the order they were laid out.
    pub tiles: Vec<String>,
}

/// Composes `tiles` — paths relative to `root`, in order — into one image.
///
/// Every tile has to be exactly one cell square, which is the size
/// `download_texture_pack` normalizes to. A tile that is not says so by name
/// rather than being scaled silently: a pack image at the wrong size is a
/// mistake upstream, and resizing it here would hide which one.
pub fn compose(root: &std::path::Path, tiles: &[String]) -> Result<Sheet, String> {
    let rows = tiles.len().div_ceil(COLUMNS as usize) as u32;
    // Opaque black, not transparent: a tile that failed to cover its cell
    // would otherwise blend with whatever the renderer draws under it, and a
    // black square is visible.
    let mut sheet = RgbaImage::from_pixel(COLUMNS * CELL, rows.max(1) * CELL, image::Rgba([0, 0, 0, 255]));

    for (n, tile) in tiles.iter().enumerate() {
        let path = root.join(tile);
        let img = image::open(&path).map_err(|e| format!("{tile}: {e}"))?.to_rgba8();
        if img.width() != CELL || img.height() != CELL {
            return Err(format!("{tile} is {}x{}, not {CELL} square", img.width(), img.height()));
        }
        let (x, y) = ((n as u32 % COLUMNS) * CELL, (n as u32 / COLUMNS) * CELL);
        image::imageops::replace(&mut sheet, &img, x as i64, y as i64);
    }

    let encoded = webp::Encoder::from_rgba(sheet.as_raw(), sheet.width(), sheet.height()).encode(QUALITY);
    Ok(Sheet {
        bytes: encoded.to_vec(),
        tiles: tiles.to_vec(),
    })
}
