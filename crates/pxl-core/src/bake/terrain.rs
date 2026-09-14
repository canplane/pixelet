//! A terrain page's chunk as the render pipeline reads it:
//! render/layer/terrain/{elevation,adapter}.ts, in Rust.
//!
//! A page is a heightmap — four ground channels of one type byte a cell,
//! and a coarse elevation raster sampled bicubically at cell centres to the
//! chunk's 64×64 — and a chunk of it becomes, cell by cell, one material
//! (the last non-void channel wins, in the channels' fixed order) and one
//! top. That is what the terrain layer paints into a volume, one solid
//! column a cell from a floor shared by the chunk, and what the direct path
//! writes as one run a column without a volume. Why the floor is
//! band-aligned, why water-family cells sit a cell lower, and why the
//! sampling is what it is are the TypeScript's, mirrored so a chunk read
//! here is byte for byte the chunk read there (terrain.test.ts).

use crate::bake::volume::{Volumes, Voxel};
use crate::bake::voxel::NO_LAYER;
use xpute_core::ds::buddy::Pool;

pub const CHUNK_SPAN: u32 = 64;
const CHUNK_CELLS: usize = (CHUNK_SPAN * CHUNK_SPAN) as usize;
/// A page's cells on a side, and its elevation raster's samples on a side
/// before and after the padding the bicubic reads past the edge.
pub const PAGE_CELL_SPAN: u32 = 1024;
pub const PAGE_ELE_SPAN: u32 = 64;
pub const ELE_PAD: u32 = 2;
pub const ELE_PADDED_SPAN: u32 = PAGE_ELE_SPAN + 2 * ELE_PAD;
const ELE_CELL_STEP: f64 = PAGE_CELL_SPAN as f64 / PAGE_ELE_SPAN as f64;

/// How far below the chunk's lowest cell the shared floor sits, before it
/// is rounded down to a band.
const TERRAIN_Z_MARGIN: i32 = 4;
/// How much a water-family cell sits below its sampled height.
const WATER_RECESS_CELLS: i32 = 1;

/// The channels, in override order: a later one's non-void type wins.
pub const CHANNELS: usize = 4;
const LAND: usize = 0;
const WATER: usize = 1;
const STRUCTURE: usize = 2;
const TRANSPORT: usize = 3;

const VOID: u8 = 0;
/// land.SEA: bare land masked as water-looking, so it recesses like water.
const SEA: u8 = 1 | 0x80;

/// What a channel's type byte means: the colour and the texture-array
/// layer of each type it has, and the material class the channel paints.
#[derive(Clone, Copy)]
pub struct Channel {
    pub rgb: [[u8; 3]; 256],
    pub has_color: [bool; 256],
    pub layer: [u8; 256],
    pub has_layer: [bool; 256],
    pub class: u8,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            rgb: [[0; 3]; 256],
            has_color: [false; 256],
            layer: [0; 256],
            has_layer: [false; 256],
            class: 0,
        }
    }
}

/// The tables and the base material every cell starts from.
pub struct Materials {
    pub channels: [Channel; CHANNELS],
    pub base_rgb: [u8; 3],
    pub base_layer: u8,
    pub base_class: u8,
}

impl Default for Materials {
    fn default() -> Self {
        Materials {
            channels: [Channel::default(); CHANNELS],
            base_rgb: [0; 3],
            base_layer: NO_LAYER as u8,
            base_class: 0,
        }
    }
}

/// One page's lanes, as the client keeps them in the core memory.
pub struct PageView<'a> {
    pub ground: [&'a [u8]; CHANNELS],
    pub ele: &'a [i16],
}

/// JavaScript's Math.round: ties toward +∞, where Rust's go away from zero.
pub fn js_round(x: f64) -> f64 {
    if x.fract() == -0.5 {
        x.ceil()
    } else {
        x.round()
    }
}

fn clamp(x: i32, lo: i32, hi: i32) -> i32 {
    x.max(lo).min(hi)
}

/// Bicubic sample of the padded elevation raster at cell (tx, ty)'s centre.
pub fn sample_ele(page_ele: &[i16], tx: u32, ty: u32) -> i16 {
    let sx = (tx as f64 + 0.5) / ELE_CELL_STEP + ELE_PAD as f64 - 0.5;
    let sy = (ty as f64 + 0.5) / ELE_CELL_STEP + ELE_PAD as f64 - 0.5;
    let sx0 = sx.floor() as i32;
    let sy0 = sy.floor() as i32;
    let fdx = sx - sx0 as f64;
    let fdy = sy - sy0 as f64;

    let x2 = fdx * fdx;
    let x3 = x2 * fdx;
    let wx = [-0.5 * x3 + x2 - 0.5 * fdx, 1.5 * x3 - 2.5 * x2 + 1.0, -1.5 * x3 + 2.0 * x2 + 0.5 * fdx, 0.5 * x3 - 0.5 * x2];
    let y2 = fdy * fdy;
    let y3 = y2 * fdy;
    let wy = [-0.5 * y3 + y2 - 0.5 * fdy, 1.5 * y3 - 2.5 * y2 + 1.0, -1.5 * y3 + 2.0 * y2 + 0.5 * fdy, 0.5 * y3 - 0.5 * y2];

    let span = ELE_PADDED_SPAN as i32;
    let at = |x: i32, y: i32| page_ele[(y * span + x) as usize] as f64;

    if sx0 >= 1 && sx0 <= span - 3 && sy0 >= 1 && sy0 <= span - 3 {
        let row = |y: i32| at(sx0 - 1, y) * wx[0] + at(sx0, y) * wx[1] + at(sx0 + 1, y) * wx[2] + at(sx0 + 2, y) * wx[3];
        let out = row(sy0 - 1) * wy[0] + row(sy0) * wy[1] + row(sy0 + 1) * wy[2] + row(sy0 + 2) * wy[3];
        return js_round(out) as i16;
    }

    let mut out = 0.0;
    for j in 0..4 {
        let py = clamp(sy0 - 1 + j, 0, span - 1);
        let mut row_out = 0.0;
        for i in 0..4 {
            let px = clamp(sx0 - 1 + i, 0, span - 1);
            row_out += at(px, py) * wx[i as usize];
        }
        out += row_out * wy[j as usize];
    }
    js_round(out) as i16
}

/// One chunk of a page, sampled: a material and a top a cell, and the
/// floor the columns share.
pub struct ChunkSample {
    /// Packed 0xCC_RRGGBB a cell.
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    /// The elevation a cell, in cells; a column's top is one below it.
    pub ele: Vec<i16>,
    pub floor: i32,
}

impl Default for ChunkSample {
    fn default() -> Self {
        ChunkSample {
            material: vec![0; CHUNK_CELLS],
            layer_idx: vec![0; CHUNK_CELLS],
            ele: vec![0; CHUNK_CELLS],
            floor: 0,
        }
    }
}

impl ChunkSample {
    /// Samples the chunk at page cell (tx0, ty0). `show_structures` off
    /// skips the structure channel, as the setting does.
    pub fn read(&mut self, page: &PageView, tables: &Materials, tx0: u32, ty0: u32, show_structures: bool) {
        let mut min_ele = i32::MAX;
        for dy in 0..CHUNK_SPAN {
            for dx in 0..CHUNK_SPAN {
                let i = (dy * CHUNK_SPAN + dx) as usize;
                let (tx, ty) = (tx0 + dx, ty0 + dy);
                let page_idx = (ty * PAGE_CELL_SPAN + tx) as usize;
                let types = [
                    page.ground[LAND][page_idx],
                    page.ground[WATER][page_idx],
                    page.ground[STRUCTURE][page_idx],
                    page.ground[TRANSPORT][page_idx],
                ];

                // The material: the base, then each channel in order.
                let (mut rgb, mut class, mut layer) = (tables.base_rgb, tables.base_class, tables.base_layer);
                for (ch, &t) in types.iter().enumerate() {
                    if ch == STRUCTURE && !show_structures {
                        continue;
                    }
                    if t == VOID {
                        continue;
                    }
                    let c = &tables.channels[ch];
                    if !c.has_color[t as usize] {
                        continue;
                    }
                    rgb = c.rgb[t as usize];
                    class = c.class;
                    layer = if c.has_layer[t as usize] { c.layer[t as usize] } else { NO_LAYER as u8 };
                }
                self.material[i] = ((class as u32) << 24) | ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32;
                self.layer_idx[i] = layer;

                // The height, a cell lower for water-family ground: the water
                // channel, or bare land masked as sea, under nothing built.
                let mut ele = sample_ele(page.ele, tx, ty) as i32;
                let water_family = types[STRUCTURE] == VOID && types[TRANSPORT] == VOID && (types[WATER] != VOID || types[LAND] == SEA);
                if water_family {
                    ele -= WATER_RECESS_CELLS;
                }
                self.ele[i] = ele as i16;
                min_ele = min_ele.min(ele);
            }
        }
        // One band-aligned floor shared by the whole chunk, under every peak.
        self.floor = (min_ele - TERRAIN_Z_MARGIN).div_euclid(CHUNK_SPAN as i32) * CHUNK_SPAN as i32;
    }

    /// Paints every column into `volume`, floor to top.
    pub fn paint(&self, volumes: &mut Volumes, pools: &mut [Pool], volume: usize) -> bool {
        for cy in 0..CHUNK_SPAN {
            for cx in 0..CHUNK_SPAN {
                let i = (cy * CHUNK_SPAN + cx) as usize;
                let v = Voxel {
                    material: self.material[i],
                    layer_idx: self.layer_idx[i],
                    sub: 0,
                };
                if !volumes.fill_column(pools, volume, (cx, cy), self.floor, self.ele[i] as i32 - 1, v) {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_as_javascript_does() {
        // Math.round takes a tie toward +∞; f64::round takes it away from zero.
        assert_eq!(js_round(2.5), 3.0);
        assert_eq!(js_round(-2.5), -2.0);
        assert_eq!(js_round(-2.4), -2.0);
        assert_eq!(js_round(-2.6), -3.0);
        assert_eq!(js_round(-0.5), 0.0);
    }
}
