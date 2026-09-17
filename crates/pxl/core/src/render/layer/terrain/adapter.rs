// pxl_core/render/layer/terrain/adapter.rs

//! A terrain page's chunk as the render pipeline reads it.
//!
//! A page is a heightmap — four ground channels of one type byte a cell,
//! and a coarse elevation raster sampled bicubically at cell centres to the
//! chunk's 64×64 — and a chunk of it becomes, cell by cell, one material
//! (the last non-void channel wins, in the channels' fixed order) and one
//! top. That is what the terrain layer paints into a volume, one solid
//! column a cell from a floor shared by the chunk, and what the direct path
//! writes as one run a column without a volume.
//!
//! The floor is rounded down to a whole band (`min_ele - TERRAIN_Z_MARGIN`,
//! floored to CHUNK_CELL_SPAN) because a volume allocates in whole bands: a floor
//! anywhere inside one would open a band below it for the few voxels that
//! cross. Water-family cells sit WATER_RECESS_CELLS below their sampled
//! height so a shoreline reads as water meeting land rather than as two
//! surfaces at the same level.

use pxl_shared::bake::constant::{CHUNK_CELL_SPAN, PAGE_CELL_SPAN};

use crate::render::layer::pipeline::volume::{Volumes, Voxel, NO_LAYER};
use crate::render::layer::terrain::elevation::{sample_ele, WATER_RECESS_CELLS};

/// How far below the chunk's lowest cell the shared floor sits, before it
/// is rounded down to a band.
const TERRAIN_Z_MARGIN: i32 = 4;
const CHUNK_CELL_SPAN2: usize = (CHUNK_CELL_SPAN * CHUNK_CELL_SPAN) as usize;

/// The channels, in override order: a later one's non-void type wins.
pub const CHANNELS: usize = 4;
const LAND: usize = 0;
const WATER: usize = 1;
const STRUCTURE: usize = 2;
const TRANSPORT: usize = 3;

const VOID: u8 = 0;
/// land.sea: bare land masked as water-looking, so it recesses like water.
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
            material: vec![0; CHUNK_CELL_SPAN2],
            layer_idx: vec![0; CHUNK_CELL_SPAN2],
            ele: vec![0; CHUNK_CELL_SPAN2],
            floor: 0,
        }
    }
}

impl ChunkSample {
    /// Samples the chunk at page cell (tx0, ty0). `show_structures` off
    /// skips the structure channel, as the setting does.
    pub fn read(&mut self, page: &PageView, tables: &Materials, tx0: u32, ty0: u32, show_structures: bool) {
        let mut min_ele = i32::MAX;
        for dy in 0..CHUNK_CELL_SPAN {
            for dx in 0..CHUNK_CELL_SPAN {
                let i = (dy * CHUNK_CELL_SPAN + dx) as usize;
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
                    ele -= WATER_RECESS_CELLS as i32;
                }
                self.ele[i] = ele as i16;
                min_ele = min_ele.min(ele);
            }
        }
        // One band-aligned floor shared by the whole chunk, under every peak.
        self.floor = (min_ele - TERRAIN_Z_MARGIN).div_euclid(CHUNK_CELL_SPAN as i32) * CHUNK_CELL_SPAN as i32;
    }

    /// Paints every column into `volume`, floor to top.
    pub fn paint(&self, volumes: &mut Volumes, volume: usize) -> bool {
        for cy in 0..CHUNK_CELL_SPAN {
            for cx in 0..CHUNK_CELL_SPAN {
                let i = (cy * CHUNK_CELL_SPAN + cx) as usize;
                let v = Voxel {
                    material: self.material[i],
                    layer_idx: self.layer_idx[i],
                    sub: 0,
                };
                if !volumes.fill_column(volume, (cx, cy), self.floor, self.ele[i] as i32 - 1, v) {
                    return false;
                }
            }
        }
        true
    }
}

#[cfg(test)]
#[path = "adapter.test.rs"]
mod test;
