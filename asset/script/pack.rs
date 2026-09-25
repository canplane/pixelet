// ingest-asset/pack.rs

//! The texture pack's manifest, and what one entry becomes on disk.
//!
//! `asset/cube/manifest.json` names a source URL per cube type. The originals
//! it fetches are kept at their native resolution under `asset/cube/original/`
//! and are the resize source on every later run, so a re-run never re-fetches
//! merely to re-normalize — which also means the network is only touched when
//! the manifest names a URL that has not been fetched before.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The tile edge in pixels. One shared array texture, one size for every
/// layer, so a tile that is not this cannot be a layer of it.
pub const TILE_SIZE: u32 = 256;

/// "cute-painted-fantasy-v1" sources are hand-painted well above 256² and need
/// a real smoothing resample or they re-pixelate on the way down; "legacy"
/// sources are the original pixel-art set and keep nearest-neighbor. Absent
/// means legacy — the conservative choice, since nearest never blurs a source
/// that genuinely was pixel-perfect already.
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

/// One entry as the tool works it: where it comes from, where its original is
/// cached, and where the normalized tile goes.
pub struct Item {
    /// For logging only, e.g. `terrain.top.land.bare`.
    pub label: String,
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
    let mut push = |label: String, e: &Entry| {
        items.push(Item {
            label,
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
            push(format!("terrain.top.{channel}.{ty}"), entry);
        }
    }
    for (ty, entry) in &manifest.terrain.side {
        push(format!("terrain.side.{ty}"), entry);
    }
    for (color, entry) in &manifest.objects.block {
        push(format!("objects.block.{color}"), entry);
    }
    Ok(items)
}

/// A manifest edit that repoints an existing `file` at a *different* URL has
/// to force a re-fetch of the original: the bytes on disk alone cannot reveal
/// that the manifest moved on.
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

/// A single tile source should be square. Anything else is either a
/// multi-frame sheet or a bad crop, and neither is something to stretch into a
/// tile silently.
pub fn looks_like_sprite_sheet(w: u32, h: u32) -> bool {
    w != h && w.max(h) as f64 / w.min(h) as f64 >= 1.2
}

/// Nearest-neighbor, for the pixel-art sources: their clusters have to stay
/// crisp rather than blur into a smear. Only ever called on a square source,
/// so it never distorts an aspect ratio.
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

/// No unintended transparent pixel afterward: every alpha becomes 255.
///
/// The manifest's own `paletteHex` is the flatten background where it is given
/// — the color that type is already known to read as — falling back to the
/// image's own opaque-pixel average for the one entry without one.
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
        if n > 0 {
            [(r / n) as u8, (g / n) as u8, (b / n) as u8]
        } else {
            [200, 200, 200]
        }
    });

    for px in data.as_chunks_mut::<4>().0 {
        if px[3] == 255 {
            continue;
        }
        let t = px[3] as f64 / 255.0;
        for c in 0..3 {
            px[c] = (px[c] as f64 * t + bg[c] as f64 * (1.0 - t)).round() as u8;
        }
        px[3] = 255;
    }
}
