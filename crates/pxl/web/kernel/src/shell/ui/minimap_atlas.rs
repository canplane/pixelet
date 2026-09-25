// pxl-kernel/shell/ui/minimap_atlas.rs

//! The minimap's image: the ground around the map centre's page, a cell of
//! the level whose cell is a minimap pixel to a pixel, as RGBA pixels the
//! shell puts on a canvas. That level's page is folded from the leaves by
//! majority (pxl-spec bake/fold.rs), so a pixel is the ground its 16 u square
//! mostly is rather than whichever cell a stride happened to land on.
//! Rebaked when a page of the level arrives or leaves and when the centre
//! crosses a page, not on every pan tick — the pan is `minimap_pan_px`'s.

use pxl_render::mesh::paint::palette::shader_meta::rgb255;
use pxl_spec::bake::constant::CELL;
use pxl_spec::space::hierarchy::Scale;
use pxl_spec::space::pyramid::GROUND;

use pxl_spec::bake::format::GROUND_CH_KEYS;
use pxl_spec::cell::{color, cube_types};
use pxl_spec::space::indexer::{L3, L3_CENTER, L3_IDX, L3_RANGE};
use pxl_spec::space::pvf3::ivec::{self, IVec};
use pxl_spec::space::pvf3::pvf3::{self, Pvf3};
use xpute_core::math::scalar::clamp;

use crate::control::camera::config::camera_cfg::view::DEFAULT_DIST;
use crate::global::Global;
use crate::shell::map_store::map_store;
use crate::world::pyramid::Layer as _;
use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pyramid::LevelPage;

/// The minimap's scale and reach, in units: properties of the minimap, not of
/// how wide a page is. The atlas is a whole number of leaf pages around the
/// centre's, so both are turned into pages here: at a page of 512 u, 32
/// pixels a page and five pages each side.
const MINIMAP_PX: Scale = Scale::of(4);
const ATLAS_REACH_U: u64 = 2560;

const ATLAS_PAGE_PX: u32 = MINIMAP_PX.count_in(L3);

/// The level whose cell is one minimap pixel.
const ATLAS_LEVEL: u32 = MINIMAP_PX.above(CELL) as u32;
/// Pages each side of the centre one: as many as it takes for the atlas's
/// half-width, the centre page's half included, to reach ATLAS_REACH_U.
const ATLAS_RADIUS_PAGES: u32 = ((ATLAS_REACH_U - L3_RANGE / 2).div_ceil(L3_RANGE)) as u32;

const L3_RANGE_REAL: f64 = L3_RANGE as f64;
const ATLAS_SPAN_PAGES: u32 = ATLAS_RADIUS_PAGES * 2 + 1;
const ATLAS_PX: u32 = ATLAS_SPAN_PAGES * ATLAS_PAGE_PX;

pub const MINIMAP_PX_PER_U: f64 = ATLAS_PAGE_PX as f64 / L3_RANGE_REAL;
pub const MINIMAP_ATLAS_PX: u32 = ATLAS_PX;

const MIN_MINIMAP_SCALE: f64 = 0.5;
const MAX_MINIMAP_SCALE: f64 = 2.0;

type Rgb = [u8; 3];

/// Ground channel bytes are type IDs, not weights, so a cell's colour is the
/// last non-void channel's in `GROUND_CH_KEYS`' order — the order the terrain
/// paint resolves a cell by too, which is why the minimap and the 3D terrain
/// agree about a cell without either knowing the other.
struct Colors {
    base: Rgb,
    by_channel: [[Option<Rgb>; 256]; 4],
}

fn colors() -> Colors {
    let base = color::ground(cube_types::base::CHANNEL, cube_types::base::TYPE).map_or([0; 3], |s| rgb255(s.rgb));
    let mut by_channel = [[None; 256]; 4];
    for (slot, ch) in GROUND_CH_KEYS.iter().enumerate() {
        for (type_id, rgb) in by_channel[slot].iter_mut().enumerate() {
            *rgb = color::ground(*ch, type_id as u8).map(|s| rgb255(s.rgb));
        }
    }
    Colors { base, by_channel }
}

pub struct MinimapAtlas {
    /// ATLAS_PX² RGBA pixels, north up.
    pub pixels: Vec<u8>,
    pub origin: Pvf3,
    /// Moves on every rebake, so the shell uploads once per image.
    pub revision: u32,
}

struct Atlas {
    colors: Colors,
    image: Option<MinimapAtlas>,
    baked_for: Option<(u64, IVec)>,
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
    let origin = Pvf3 { i: L3_CENTER(map_center.i), f: 0 };
    let half = ATLAS_PX as i64 / 2;
    // A pixel is a cell of ATLAS_LEVEL, named by its index at that scale.
    let [ox, oy, _] = MINIMAP_PX.index_of(&origin);
    let cell_at = |cx: i64, cy: i64| GROUND.cell_of(ATLAS_LEVEL, &MINIMAP_PX.base_of([cx, cy, 0]));
    // What the image is made of: which of the pages it reads are resident.
    // A bake's page is the same whenever it comes, so that says every
    // change there is.
    let (cx0, cy1) = (ox - half, oy + half - 1);
    let (cx1, cy0) = (cx0 + ATLAS_PX as i64 - 1, cy1 - (ATLAS_PX as i64 - 1));
    let (first, last) = (cell_at(cx0, cy0).page().page, cell_at(cx1, cy1).page().page);
    let resident: u64 = (ivec::Y(first)..=ivec::Y(last))
        .flat_map(|y| (ivec::X(first)..=ivec::X(last)).map(move |x| (x, y)))
        .enumerate()
        .filter(|&(_, (x, y))| Terrain::pyramid().get(LevelPage::new(ATLAS_LEVEL, ivec::gather(x, y, 0))).is_some())
        .fold(0, |bits, (k, _)| bits | 1 << k);
    let key = (resident, L3_IDX(map_center.i));
    if a.baked_for != Some(key) {
        a.baked_for = Some(key);
        let mut pixels = vec![0u8; (ATLAS_PX * ATLAS_PX * 4) as usize];
        for py in 0..ATLAS_PX as i64 {
            // Atlas rows run down for a north-up image, and world Y runs up:
            // the flip, or the minimap renders upside down.
            let cy = oy + (half - 1 - py);
            for px in 0..ATLAS_PX as i64 {
                let cell = cell_at(ox + (px - half), cy);
                let mut color = a.colors.base;
                if let Some(tile) = Terrain::pyramid().get(cell.page()) {
                    for (slot, ch) in GROUND_CH_KEYS.iter().enumerate() {
                        let v = tile.ground.channel(cell.index() as usize, *ch);
                        if let Some(c) = (v != 0).then(|| a.colors.by_channel[slot][v as usize]).flatten() {
                            color = c;
                        }
                    }
                }
                let at = ((py * ATLAS_PX as i64 + px) * 4) as usize;
                pixels[at..at + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
        let revision = a.image.as_ref().map_or(0, |i| i.revision + 1);
        a.image = Some(MinimapAtlas { pixels, origin, revision });
    }
    a.image.as_ref()
}

pub fn minimap_pan_px(map_center: &Pvf3, origin: &Pvf3) -> (f64, f64) {
    let mut delta = Pvf3::default();
    pvf3::sub(&mut delta, map_center, origin);
    (-pvf3::to_real(pvf3::X(&delta)) * MINIMAP_PX_PER_U, pvf3::to_real(pvf3::Y(&delta)) * MINIMAP_PX_PER_U)
}

pub fn minimap_scale_for_dist(dist: f64) -> f64 {
    clamp(DEFAULT_DIST / dist, MIN_MINIMAP_SCALE, MAX_MINIMAP_SCALE)
}
