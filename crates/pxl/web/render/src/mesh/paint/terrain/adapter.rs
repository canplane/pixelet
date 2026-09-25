// pxl-render/mesh/paint/terrain/adapter.rs

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
//! floored to BAND_CELLS) because a volume allocates in whole bands: a floor
//! anywhere inside one would open a band below it for the few voxels that
//! cross. A cell sits its type's recess below its sampled height, so a
//! shoreline reads as water meeting land rather than as two surfaces at the
//! same level.

use pxl_spec::bake::constant::BAND_CELLS;
use pxl_spec::bake::format::{GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::bake::ground::{channel_bit, ChannelMask, BASE};
use pxl_spec::bake::pack::{unpack, GroundCell};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND, PAGE_CELLS};

use crate::mesh::paint::palette::shader_meta::{ground_color_lookup, ground_layer_index_lookup};
use crate::mesh::paint::terrain::elevation::{ground_ele, GroundLanes};
use crate::mesh::volume::{MaterialClasses, Volumes, Voxel, NO_LAYER};
use crate::mesh::voxel_scan::{apron_col, APRON, COL_SPAN};
use std::sync::LazyLock;

/// How far below the chunk's lowest cell the shared floor sits, before it
/// is rounded down to a band.
const TERRAIN_Z_MARGIN: i32 = 4;
const COL_SPAN2: usize = (COL_SPAN * COL_SPAN) as usize;

/// The channels, in `GROUND_CH_KEYS`' order, which is also the order they
/// paint in: a later one's non-void type wins.
pub const CHANNELS: usize = 4;

const VOID: u8 = 0;

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

/// Every ground type's colour and layer, from the palette's tables
/// (palette/shader_meta.rs), each channel's class, and the catalog's base:
/// a function of the palette and the catalog alone, so made once, here,
/// where the paint and the minimap read it.
pub static MATERIALS: LazyLock<Materials> = LazyLock::new(Materials::of_palette);

/// The class a channel's cells are painted as.
fn class_of(ch: GroundChannelKey) -> u8 {
    (match ch {
        GroundChannelKey::LAND => MaterialClasses::LAND,
        GroundChannelKey::WATER => MaterialClasses::WATER,
        GroundChannelKey::STRUCTURE => MaterialClasses::STRUCTURE,
        GroundChannelKey::TRANSPORT => MaterialClasses::TRANSPORT,
    }) as u8
}

impl Materials {
    fn of_palette() -> Materials {
        let rgb_of = |ch, t: usize| ground_color_lookup(ch).filter(|c| c.has[t] != 0).map(|c| [c.rgb[t * 3], c.rgb[t * 3 + 1], c.rgb[t * 3 + 2]]);
        let layer_of = |ch, t: usize| ground_layer_index_lookup(ch).filter(|l| l.has[t] != 0).map(|l| l.index[t]);
        let base = BASE.type_id as usize;
        let mut m = Materials {
            channels: [Channel::default(); CHANNELS],
            base_rgb: rgb_of(BASE.channel, base).unwrap_or([0; 3]),
            base_layer: layer_of(BASE.channel, base).unwrap_or(NO_LAYER as u8),
            base_class: class_of(BASE.channel),
        };
        for (dst, ch) in m.channels.iter_mut().zip(GROUND_CH_KEYS) {
            dst.class = class_of(ch);
            for t in 0..256 {
                if let Some(rgb) = rgb_of(ch, t) {
                    (dst.rgb[t], dst.has_color[t]) = (rgb, true);
                }
                if let Some(layer) = layer_of(ch, t) {
                    (dst.layer[t], dst.has_layer[t]) = (layer, true);
                }
            }
        }
        m
    }
}

/// One page's lanes, as the client keeps them in the core memory.
pub struct PageView<'a> {
    /// The page's ground, a cell's four channels packed into one
    /// `GroundCell` (pxl-spec bake/pack.rs), in the lattice's cell order.
    pub cells: &'a [GroundCell],
    pub ele: &'a [i16],
}

/// One chunk of a page, sampled: a material and a top a cell, and the
/// floor the columns share, over the run set's padded column grid: the
/// chunk's own cells and the ring of neighbours around them (voxel_scan.rs's APRON). The
/// ring is sampled from the same page — the elevation raster is padded, so a
/// cell outside the page still has a height — and is never painted into the
/// volume or emitted; it is there for the face mask to ask about.
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
            material: vec![0; COL_SPAN2],
            layer_idx: vec![0; COL_SPAN2],
            ele: vec![0; COL_SPAN2],
            floor: 0,
        }
    }
}

impl ChunkSample {
    /// Samples the chunk at page cell (tx0, ty0). A channel in `hidden` is
    /// stepped over, as `shown_ground` steps over it.
    pub fn read(&mut self, page: &PageView, tx0: u32, ty0: u32, hidden: ChannelMask) {
        let tables = &*MATERIALS;
        let skip = GROUND_CH_KEYS.map(|k| hidden & channel_bit(k) != 0);
        let mut min_ele = i32::MAX;
        let last = PAGE_CELLS as i32 - 1;
        for py in 0..COL_SPAN {
            for px in 0..COL_SPAN {
                let i = (py * COL_SPAN + px) as usize;
                let own = APRON..COL_SPAN - APRON;
                let inner = own.contains(&px) && own.contains(&py);
                let (tx, ty) = (tx0 as i32 + px as i32 - APRON as i32, ty0 as i32 + py as i32 - APRON as i32);
                // The ground channels are the page's own span; only the
                // elevation is padded past it. An apron cell outside the
                // page keeps the height — which is all the face mask reads
                // — and takes the base material, which nothing draws.
                let on_page = (0..=last).contains(&tx) && (0..=last).contains(&ty);
                let types = if on_page {
                    unpack(page.cells[GROUND.cell_index(tx as u32, ty as u32) as usize])
                } else {
                    [VOID; 4]
                };

                // The material: the base, then each channel in order.
                let (mut rgb, mut class, mut layer) = (tables.base_rgb, tables.base_class, tables.base_layer);
                for (ch, &t) in types.iter().enumerate() {
                    if skip[ch] || t == VOID {
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

                // The height every consumer of this page takes, recess and
                // all (elevation.rs's ground_ele). An apron cell off the page
                // has no channels to judge by and takes the raw height.
                let lanes = on_page.then_some(GroundLanes { cells: page.cells });
                let ele = ground_ele(page.ele, lanes, tx, ty) as i32;
                self.ele[i] = ele as i16;
                // The floor is the chunk's own: an apron column is written
                // from it, and a neighbour's valley must not drop it.
                if inner {
                    min_ele = min_ele.min(ele);
                }
            }
        }
        // One band-aligned floor shared by the whole chunk, under every peak.
        self.floor = (min_ele - TERRAIN_Z_MARGIN).div_euclid(BAND_CELLS as i32) * BAND_CELLS as i32;
    }

    /// Paints the chunk's own columns into `volume`, floor to top. The
    /// apron's are not painted: the volume is the chunk's, and they exist
    /// only as columns of the run set.
    pub fn paint(&self, volumes: &mut Volumes, volume: u32) -> bool {
        for cy in 0..CHUNK_CELLS {
            for cx in 0..CHUNK_CELLS {
                let i = apron_col(cx, cy) as usize;
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
