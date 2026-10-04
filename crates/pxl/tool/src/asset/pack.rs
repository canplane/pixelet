// pxl-tool/asset/pack.rs

//! The texture pack's manifest. Originals are cached under
//! `asset/cube/original/` and re-fetched only when an entry's URL changes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// One size for every layer of the shared array texture.
pub const TILE_SIZE: u32 = 256;

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
            px[c] = (px[c] as f64 * t + bg[c] as f64 * (1.0 - t)).round() as u8;
        }
        px[3] = 255;
    }
}
