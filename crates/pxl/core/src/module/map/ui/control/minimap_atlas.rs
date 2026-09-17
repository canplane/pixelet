// pxl_core/module/map/ui/control/minimap_atlas.rs

//! The minimap's image: the resident pages around the map centre's page,
//! sampled every SAMPLE_STRIDE cells, as RGBA pixels the shell puts on a canvas.
//! Rebaked when a page arrives or leaves and when the centre crosses a page,
//! not on every pan tick — the pan is `minimap_pan_px`'s.

use pxl_render::layer::terrain::shader_meta::rgb255;
use pxl_shared::bake::constant::PAGE_CELL_SPAN;
use pxl_shared::bake::format::GROUND_CH_KEYS;
use pxl_shared::spec::cell::{cube_colors, cube_types};
use pxl_shared::spec::space::indexer::{FROM_L3_IDX, L3_CENTER, L3_IDX, L3_RANGE};
use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};
use xpute_core::kit::math::clamp;

use crate::module::camera::config::camera_cfg::view::DEFAULT_DIST;
use crate::module::map::store::map_store;
use crate::module::world::terrain::store::tile_pages;
use crate::sys::global::Global;

const ATLAS_PAGE_PX: u32 = 64;
const SAMPLE_STRIDE: u32 = PAGE_CELL_SPAN / ATLAS_PAGE_PX;
const ATLAS_RADIUS_PAGES: u32 = 2;

const L3_RANGE_REAL: f64 = L3_RANGE as f64;
const ATLAS_SPAN_PAGES: u32 = ATLAS_RADIUS_PAGES * 2 + 1;
const ATLAS_PX: u32 = ATLAS_SPAN_PAGES * ATLAS_PAGE_PX;
const ATLAS_CENTER_PX: f64 = ATLAS_PX as f64 / 2.0;

pub const MINIMAP_PX_PER_U: f64 = ATLAS_PAGE_PX as f64 / L3_RANGE_REAL;
pub const MINIMAP_ATLAS_PX: u32 = ATLAS_PX;

const MIN_MINIMAP_SCALE: f64 = 0.5;
const MAX_MINIMAP_SCALE: f64 = 2.0;

type Rgb = [u8; 3];

/// Ground channel bytes are type IDs, not weights, so this must reuse the same
/// per-channel override order the terrain paint resolves a cell by
/// (crates/pxl/render/src/layer/terrain/adapter.rs) — reordering GROUND_CH_KEYS here would
/// silently mismatch the 3D terrain's colors.
struct Colors {
    base: Rgb,
    by_channel: [[Option<Rgb>; 256]; 4],
}

fn colors() -> Colors {
    let base = cube_colors
        .get("land")
        .and_then(|c| c.get(&(cube_types::land::bare as u32)).copied().flatten())
        .map_or([0; 3], |s| rgb255(s.rgb));
    let mut by_channel = [[None; 256]; 4];
    for (slot, ch) in GROUND_CH_KEYS.iter().enumerate() {
        if let Some(table) = cube_colors.get(ch.as_str()) {
            for (&type_id, swatch) in table {
                if let (Some(swatch), true) = (swatch, type_id < 256) {
                    by_channel[slot][type_id as usize] = Some(rgb255(swatch.rgb));
                }
            }
        }
    }
    Colors { base, by_channel }
}

fn page_rel_u(page_idx: IVec, origin: &PVF3) -> (f64, f64) {
    let mut delta = PVF3::default();
    pvf3::sub(&mut delta, &PVF3 { i: FROM_L3_IDX(page_idx), f: 0 }, origin);
    (pvf3::to_real(pvf3::X(&delta)), pvf3::to_real(pvf3::Y(&delta)))
}

pub struct MinimapAtlas {
    /// ATLAS_PX² RGBA pixels, north up.
    pub pixels: Vec<u8>,
    pub origin: PVF3,
    /// Moves on every rebake, so the shell uploads once per image.
    pub revision: u32,
}

struct Atlas {
    colors: Colors,
    image: Option<MinimapAtlas>,
    baked_for: Option<(u32, IVec)>,
}

static ATLAS: Global<Atlas> = Global::new();

/// The minimap's image as it stands, rebaked when its inputs moved.
pub fn minimap_atlas() -> Option<&'static MinimapAtlas> {
    let a = ATLAS.get(|| Atlas {
        colors: colors(),
        image: None,
        baked_for: None,
    });
    let map_center = map_store().map_center?;
    let key = (tile_pages().revision(), L3_IDX(map_center.i));
    if a.baked_for != Some(key) {
        a.baked_for = Some(key);
        let origin = PVF3 { i: L3_CENTER(map_center.i), f: 0 };
        let mut pixels = vec![0u8; (ATLAS_PX * ATLAS_PX * 4) as usize];
        for px in pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&[a.colors.base[0], a.colors.base[1], a.colors.base[2], 255]);
        }
        let max_rel_u = (ATLAS_RADIUS_PAGES as f64 + 0.5) * L3_RANGE_REAL;
        for (page_idx, tile) in tile_pages().entries() {
            let (rx, ry) = page_rel_u(page_idx, &origin);
            if rx.abs() > max_rel_u || ry.abs() > max_rel_u {
                continue;
            }
            let px0 = (ATLAS_CENTER_PX + rx * MINIMAP_PX_PER_U) as i64;
            let py0 = (ATLAS_CENTER_PX - ry * MINIMAP_PX_PER_U - ATLAS_PAGE_PX as f64) as i64;
            let ground = &tile.terrain.ground;
            for sy in 0..ATLAS_PAGE_PX {
                // Source raster rows increase with world Y; atlas pixel rows
                // increase downward for a north-up image — flip, or the minimap
                // renders upside down.
                let py = py0 + (ATLAS_PAGE_PX - 1 - sy) as i64;
                let cy = sy * SAMPLE_STRIDE;
                for sx in 0..ATLAS_PAGE_PX {
                    let off = (cy * PAGE_CELL_SPAN + sx * SAMPLE_STRIDE) as usize;
                    let mut color = a.colors.base;
                    for (slot, ch) in GROUND_CH_KEYS.iter().enumerate() {
                        let v = ground.get(*ch).get(off);
                        if v == 0 {
                            continue;
                        }
                        if let Some(c) = a.colors.by_channel[slot][v as usize] {
                            color = c;
                        }
                    }
                    let x = px0 + sx as i64;
                    if !(0..ATLAS_PX as i64).contains(&x) || !(0..ATLAS_PX as i64).contains(&py) {
                        continue;
                    }
                    let at = ((py as u32 * ATLAS_PX + x as u32) * 4) as usize;
                    pixels[at..at + 3].copy_from_slice(&color);
                }
            }
        }
        let revision = a.image.as_ref().map_or(0, |i| i.revision + 1);
        a.image = Some(MinimapAtlas { pixels, origin, revision });
    }
    a.image.as_ref()
}

pub fn minimap_pan_px(map_center: &PVF3, origin: &PVF3) -> (f64, f64) {
    let mut delta = PVF3::default();
    pvf3::sub(&mut delta, map_center, origin);
    (-pvf3::to_real(pvf3::X(&delta)) * MINIMAP_PX_PER_U, pvf3::to_real(pvf3::Y(&delta)) * MINIMAP_PX_PER_U)
}

pub fn minimap_scale_for_dist(dist: f64) -> f64 {
    clamp(DEFAULT_DIST / dist, MIN_MINIMAP_SCALE, MAX_MINIMAP_SCALE)
}
