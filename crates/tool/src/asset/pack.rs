// pxl-tool/asset/pack.rs

//! The texture pack: its manifest, and the one image it ships as. Originals are cached under
//! `asset/cube/original/` and re-fetched only when an entry's URL changes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use image::RgbaImage;
use serde::Deserialize;

/// One size for every layer of the shared array texture.
pub const TILE_SIZE: u32 = 256;
/// Tiles a row of the laid-out image.
pub const COLUMNS: u32 = 8;

/// Painted sources are far above 256² and need a smoothing resample; legacy
/// pixel art keeps nearest-neighbor, the default.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
pub enum Style {
    #[serde(rename = "cute-painted-fantasy-v1")]
    Painted,
    #[default]
    #[serde(rename = "legacy")]
    Legacy,
}

#[derive(Deserialize)]
struct Entry {
    url: String,
    file: String,
    #[serde(rename = "paletteHex")]
    palette_hex: Option<String>,
    #[serde(default)]
    style: Style,
}

#[derive(Deserialize)]
struct Terrain {
    top: BTreeMap<String, BTreeMap<String, Entry>>,
    side: BTreeMap<String, Entry>,
    #[serde(default)]
    bottom: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
struct Objects {
    block: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
struct Manifest {
    terrain: Terrain,
    objects: Objects,
}

/// Which cube face an entry is, by the manifest's keys; a placed block is the
/// `block` channel's top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Slot {
    Top { channel: String, ty: String },
    Side { ty: String },
    Bottom { ty: String },
}

pub struct Item {
    pub label: String,
    pub slot: Slot,
    pub url: String,
    pub file: String,
    pub dest: PathBuf,
    pub original: PathBuf,
    pub palette_hex: Option<String>,
    pub style: Style,
}

pub fn load(cube_dir: &Path) -> Result<Vec<Item>, String> {
    let text = std::fs::read_to_string(cube_dir.join("manifest.json")).map_err(|e| format!("manifest.json: {e}"))?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| format!("manifest.json: {e}"))?;
    let originals = cube_dir.join("original");

    let mut items = Vec::new();
    let mut push = |label: String, slot: Slot, e: &Entry| {
        items.push(Item {
            label,
            slot,
            url: e.url.clone(),
            file: e.file.clone(),
            dest: cube_dir.join(&e.file),
            original: originals.join(&e.file),
            palette_hex: e.palette_hex.clone(),
            style: e.style,
        });
    };

    for (channel, types) in &manifest.terrain.top {
        for (ty, entry) in types {
            push(
                format!("terrain.top.{channel}.{ty}"),
                Slot::Top {
                    channel: channel.clone(),
                    ty: ty.clone(),
                },
                entry,
            );
        }
    }
    // A side and a bottom come together (Minecraft's grass block): a side alone
    // leaves the cells below the surface nothing right to wear.
    if let Some(ty) = manifest.terrain.side.keys().find(|ty| !manifest.terrain.bottom.contains_key(*ty)) {
        return Err(format!("manifest.json: terrain.side.{ty} has no terrain.bottom.{ty}"));
    }
    for (ty, entry) in &manifest.terrain.side {
        push(format!("terrain.side.{ty}"), Slot::Side { ty: ty.clone() }, entry);
    }
    for (ty, entry) in &manifest.terrain.bottom {
        push(format!("terrain.bottom.{ty}"), Slot::Bottom { ty: ty.clone() }, entry);
    }
    for (ty, entry) in &manifest.objects.block {
        push(
            format!("objects.block.{ty}"),
            Slot::Top {
                channel: "block".to_string(),
                ty: ty.clone(),
            },
            entry,
        );
    }
    Ok(items)
}

/// The URL each original was fetched from: a manifest that repoints an entry
/// must force a re-fetch, which the bytes on disk cannot reveal.
pub type State = BTreeMap<String, String>;

pub fn read_state(cube_dir: &Path) -> State {
    std::fs::read_to_string(cube_dir.join(".download_state.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn write_state(cube_dir: &Path, state: &State) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    text.push('\n');
    std::fs::write(cube_dir.join(".download_state.json"), text).map_err(|e| e.to_string())
}

/// A non-square source is a sprite sheet or a bad crop; never stretch it.
pub fn looks_like_sprite_sheet(w: u32, h: u32) -> bool {
    w != h && w.max(h) as f64 / w.min(h) as f64 >= 1.2
}

pub fn nearest_resize(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    let mut dst = vec![0u8; (dw * dh * 4) as usize];
    for y in 0..dh {
        let sy = ((y as f64 + 0.5) * (sh as f64 / dh as f64)).floor() as u32;
        let sy = sy.min(sh - 1);
        for x in 0..dw {
            let sx = ((x as f64 + 0.5) * (sw as f64 / dw as f64)).floor() as u32;
            let sx = sx.min(sw - 1);
            let s = ((sy * sw + sx) * 4) as usize;
            let d = ((y * dw + x) * 4) as usize;
            dst[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    dst
}

fn hex_to_rgb(hex: &str) -> Option<[u8; 3]> {
    let h = hex.trim_start_matches('#');
    if h.len() < 6 {
        return None;
    }
    Some([u8::from_str_radix(&h[0..2], 16).ok()?, u8::from_str_radix(&h[2..4], 16).ok()?, u8::from_str_radix(&h[4..6], 16).ok()?])
}

/// Makes every alpha 255, flattening over the manifest's `paletteHex`, or the
/// opaque-pixel average where none is given.
pub fn flatten_alpha(data: &mut [u8], palette_hex: Option<&str>) {
    let bg = palette_hex.and_then(hex_to_rgb).unwrap_or_else(|| {
        let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
        for px in data.as_chunks::<4>().0 {
            if px[3] == 255 {
                r += px[0] as u64;
                g += px[1] as u64;
                b += px[2] as u64;
                n += 1;
            }
        }
        [r, g, b].map(|sum| sum.checked_div(n).map_or(200, |mean| mean as u8))
    });

    for px in data.as_chunks_mut::<4>().0 {
        if px[3] == 255 {
            continue;
        }
        let t = px[3] as f64 / 255.0;
        for c in 0..3 {
            px[c] = (px[c] as f64 * t + bg[c] as f64 * (1.0 - t)).round_ties_even() as u8;
        }
        px[3] = 255;
    }
}

const QUALITY: f32 = 92.0;

/// How far a top's mean may stay from its look's Carto color, in CIE76 ΔE:
/// the painted look kept up close, the map still Carto's from afar, where a
/// textured look is drawn as its tile's mean (pxl-render mesh/paint/palette.rs).
pub const TOWARD_CARTO: f64 = 20.0;

/// sRGB bytes to CIE L*a*b* (D65), through linear light.
fn lab_of(rgb: [u8; 3]) -> [f64; 3] {
    let [r, g, b] = rgb.map(srgb_to_linear);
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.950_47;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.088_83;
    let f = |t: f64| if t > 0.008_856 { libm::cbrt(t) } else { 7.787 * t + 16.0 / 116.0 };
    [116.0 * f(y) - 16.0, 500.0 * (f(x) - f(y)), 200.0 * (f(y) - f(z))]
}

fn rgb_of([l, a, b]: [f64; 3]) -> [u8; 3] {
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let g = |t: f64| if t * t * t > 0.008_856 { t * t * t } else { (t - 16.0 / 116.0) / 7.787 };
    let (x, y, z) = (g(fx) * 0.950_47, g(fy), g(fz) * 1.088_83);
    [3.2406 * x - 1.5372 * y - 0.4986 * z, -0.9689 * x + 1.8758 * y + 0.0415 * z, 0.0557 * x - 0.2040 * y + 1.0570 * z].map(|c| linear_to_srgb(c.clamp(0.0, 1.0)))
}

/// Moves a tile's colors, every pixel by one offset in L*a*b*, so its mean is
/// no farther than `TOWARD_CARTO` from `carto`: its pattern, the pixels'
/// differences, is left as painted. A tile already that close is left alone.
pub fn toward_carto(img: &mut RgbaImage, carto: u32) {
    let labs: Vec<[f64; 3]> = img.pixels().map(|p| lab_of([p.0[0], p.0[1], p.0[2]])).collect();
    let n = labs.len() as f64;
    let mean: [f64; 3] = core::array::from_fn(|k| labs.iter().map(|p| p[k]).sum::<f64>() / n);
    let target = lab_of([(carto >> 16) as u8, (carto >> 8) as u8, carto as u8]);
    let d = libm::sqrt((0..3).map(|k| (target[k] - mean[k]) * (target[k] - mean[k])).sum());
    if d <= TOWARD_CARTO {
        return;
    }
    let t = (d - TOWARD_CARTO) / d;
    let offset: [f64; 3] = core::array::from_fn(|k| (target[k] - mean[k]) * t);
    for (px, lab) in img.pixels_mut().zip(labs) {
        let [r, g, b] = rgb_of(core::array::from_fn(|k| lab[k] + offset[k]));
        px.0 = [r, g, b, px.0[3]];
    }
}

/// The pack laid out as one lossy WebP (q92): about 400 KB against several MB
/// lossless, downloaded on every cold start.
pub struct Image {
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
        libm::pow((c + 0.055) / 1.055, 2.4)
    }
}

fn linear_to_srgb(c: f64) -> u8 {
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * libm::pow(c, 1.0 / 2.4) - 0.055 };
    (s * 255.0).round_ties_even().clamp(0.0, 255.0) as u8
}

/// Every tile must be exactly `TILE_SIZE` square; a wrong size is an upstream mistake
/// and is refused rather than resized. A tile with a Carto color in `carto` is
/// drawn toward it first.
pub fn compose(root: &std::path::Path, tiles: &[String], carto: &[Option<u32>]) -> Result<Image, String> {
    let rows = tiles.len().div_ceil(COLUMNS as usize) as u32;
    // Opaque black, so an uncovered cell is visible rather than blended.
    let mut laid = RgbaImage::from_pixel(COLUMNS * TILE_SIZE, rows.max(1) * TILE_SIZE, image::Rgba([0, 0, 0, 255]));
    let mut means = Vec::with_capacity(tiles.len());

    for (n, tile) in tiles.iter().enumerate() {
        let path = root.join(tile);
        let mut img = image::open(&path).map_err(|e| format!("{tile}: {e}"))?.to_rgba8();
        if img.width() != TILE_SIZE || img.height() != TILE_SIZE {
            return Err(format!("{tile} is {}x{}, not {TILE_SIZE} square", img.width(), img.height()));
        }
        if let Some(&Some(color)) = carto.get(n) {
            toward_carto(&mut img, color);
        }
        let (x, y) = ((n as u32 % COLUMNS) * TILE_SIZE, (n as u32 / COLUMNS) * TILE_SIZE);
        means.push(mean_of(&img));
        image::imageops::replace(&mut laid, &img, x as i64, y as i64);
    }

    let encoded = webp::Encoder::from_rgba(laid.as_raw(), laid.width(), laid.height()).encode(QUALITY);
    Ok(Image {
        bytes: encoded.to_vec(),
        tiles: tiles.to_vec(),
        means,
    })
}
