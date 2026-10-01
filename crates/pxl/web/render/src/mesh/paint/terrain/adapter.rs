// pxl-render/mesh/paint/terrain/adapter.rs

//! A terrain page's chunk as the render pipeline reads it.
//!
//! A page is a heightmap — four ground channels of one type byte a cell,
//! and a coarse elevation raster sampled bicubically at cell centers to the
//! chunk's 64×64 — and a chunk of it becomes, cell by cell, one material
//! (the last non-void channel wins, in the channels' fixed order) and one
//! top. That is what the terrain layer paints into a volume, one solid
//! column a cell from a floor shared by the chunk, and what the direct path
//! writes as one run a column without a volume.
//!
//! A page's decks (pxl-spec bake/deck.rs) stand off that column: a deck over
//! a cell is a slab a cell thick under its top, and where a pier carries it,
//! the pier from the ground to the slab, so a column holds its ground and
//! whatever decks cross over it, each a run of its own. A deck at or under
//! the ground there — where a bridge comes down to its ends — is the way on
//! the ground, painted as the column's own material.
//!
//! The floor is rounded down to a whole band (`min_ele - TERRAIN_Z_MARGIN`,
//! floored to BAND_CELLS) because a volume allocates in whole bands: a floor
//! anywhere inside one would open a band below it for the few voxels that
//! cross. A cell sits its type's recess below its sampled height, so a
//! shoreline reads as water meeting land rather than as two surfaces at the
//! same level.

use pxl_spec::bake::constant::BAND_CELLS;
use pxl_spec::bake::deck::{Decks, Over};
use pxl_spec::bake::format::{ch_index, GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::bake::ground::{channel_bit, ChannelMask, BASE};
use pxl_spec::bake::pack::{unpack, GroundCell};
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND, PAGE_CELLS};

use crate::mesh::paint::palette::shader_meta::{ground_color_lookup, ground_layer_index_lookup};
use crate::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use crate::mesh::volume::{MaterialClasses, Volumes, Voxel, NO_LAYER};
use crate::mesh::{try_push, Refused};
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

/// What a channel's type byte means: the color and the texture-array
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

/// Every ground type's color and layer, from the palette's tables
/// (palette/shader_meta.rs), each channel's class, and the catalog's base:
/// a function of the palette and the catalog alone, so made once, here,
/// where the paint reads it.
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
    /// A ground type's voxel material, packed `0xCC_RRGGBB`, and its layer;
    /// none for a type the palette gives no color.
    pub fn of(&self, ch: GroundChannelKey, t: u8) -> Option<(u32, u8)> {
        let c = &self.channels[ch_index(ch)];
        let t = t as usize;
        c.has_color[t].then(|| (pack(c.class, c.rgb[t]), if c.has_layer[t] { c.layer[t] } else { NO_LAYER as u8 }))
    }

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

/// The decks over a chunk's own columns, from the page they are drawn from:
/// a column, a deck and what it is there.
fn collect_decks(over: &mut Vec<(usize, usize, Over)>, own: Option<&PageSource>, first: [i64; 2]) -> Result<(), Refused> {
    let Some(s) = own else { return Ok(()) };
    let n = f64::from(s.n);
    let origin = [0, 1].map(|k| (s.origin[k] + first[k]) as f64 / n);
    let side = CHUNK_CELLS as i64;
    let mut grown = Ok(());
    s.page.decks.over_grid(origin, 1.0 / n, [side, side], |cx, cy, d, o| {
        if grown.is_ok() {
            grown = try_push(over, (apron_col(cx as u32, cy as u32) as usize, d, o));
        }
    });
    grown
}

/// A voxel material: the class over the color.
fn pack(class: u8, rgb: [u8; 3]) -> u32 {
    ((class as u32) << 24) | ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32
}

/// What a pier is painted as: a building's concrete, since nothing in the
/// catalog is a pier's own and the catalog is what a voxel's material is.
const PIER: (GroundChannelKey, u8) = (GroundChannelKey::STRUCTURE, cube_types::structure::building);

/// One page's lanes, as the client keeps them in the core memory.
#[derive(Clone, Copy)]
pub struct PageView<'a> {
    /// The page's ground, a cell's four channels packed into one
    /// `GroundCell` (pxl-spec bake/pack.rs), in the lattice's cell order.
    pub cells: &'a [GroundCell],
    pub ele: &'a [i16],
    pub decks: &'a Decks,
}

/// What a page's cells are read from: the page drawn in its place — its own,
/// or one over it whose cell stands for `n` × `n` of its own until its own
/// comes — and the page's first cell as its own cells from that page's
/// corner.
#[derive(Clone, Copy)]
pub struct PageSource<'a> {
    pub page: PageView<'a>,
    pub origin: [i64; 2],
    pub n: u32,
}

/// The pages a chunk's cells lie in, each with what it is drawn from: the
/// chunk's own, and those beside it its apron reaches into.
pub struct Around<'a> {
    pub own: PageSource<'a>,
    /// The page (qx, qy) from the chunk's own at `beside_index`: none where
    /// the apron does not reach or nothing is drawn there, and those cells
    /// are read from `own`, whose elevation is padded past its edge.
    pub beside: [Option<PageSource<'a>>; 8],
}

/// Where the page (qx, qy) from a chunk's own, each in -1..=1 and not both
/// zero, is kept in `Around::beside`.
pub const fn beside_index(qx: i64, qy: i64) -> usize {
    let k = (qx + 1) + 3 * (qy + 1);
    (if k > 4 { k - 1 } else { k }) as usize
}

/// One chunk of a page, sampled: a material and a top a cell, and the
/// floor the columns share, over the run set's padded column grid: the
/// chunk's own cells and the ring of neighbors around them (voxel_scan.rs's
/// APRON). A ring cell is read from what its own page is drawn from, as the
/// chunk beside draws it, so the face mask asks about the surface that is on
/// the screen there; it is never painted into the volume or emitted.
pub struct ChunkSample {
    /// Packed 0xCC_RRGGBB a cell.
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    /// The elevation a cell, in cells; a column's top is one below it.
    pub ele: Vec<i16>,
    pub floor: i32,
    /// Column `i`'s spans over its ground are `spans[span_at[i]..span_at[i + 1]]`,
    /// lowest first and none overlapping; the apron's columns have none.
    pub span_at: Vec<u32>,
    pub spans: Vec<Span>,
    /// The decks over each column as they are read, kept between reads: a
    /// column, a deck and what it is there.
    over: Vec<(usize, usize, Over)>,
}

/// A run off the ground: a deck's slab or a pier, from `lo` up to `hi`
/// exclusive, in cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub lo: i32,
    pub hi: i32,
    pub material: u32,
    pub layer: u8,
}

impl Default for ChunkSample {
    fn default() -> Self {
        ChunkSample {
            material: vec![0; COL_SPAN2],
            layer_idx: vec![0; COL_SPAN2],
            ele: vec![0; COL_SPAN2],
            floor: 0,
            span_at: vec![0; COL_SPAN2 + 1],
            spans: Vec::new(),
            over: Vec::new(),
        }
    }
}

impl ChunkSample {
    /// Samples the chunk whose first cell is `first` of its page's cells,
    /// each cell from what the page it lies in is drawn from (`Around`).
    /// Each cell takes the material of the source cell it lies in and the
    /// height of the source's surface at its own center, so a cell read from
    /// a coarser page is that page's ground at the chunk's step and nothing
    /// invented. A channel in `hidden` is stepped over, as `shown_ground`
    /// steps over it. Refused where the decks over it need more room than
    /// the heap grants.
    pub fn read(&mut self, around: &Around, first: [i64; 2], hidden: ChannelMask) -> Result<(), Refused> {
        let tables = &*MATERIALS;
        let skip = GROUND_CH_KEYS.map(|k| hidden & channel_bit(k) != 0);
        let mut min_ele = i32::MAX;
        let last = PAGE_CELLS as i32 - 1;
        for py in 0..COL_SPAN {
            for px in 0..COL_SPAN {
                let i = (py * COL_SPAN + px) as usize;
                let own = APRON..COL_SPAN - APRON;
                let inner = own.contains(&px) && own.contains(&py);
                let (x, y) = (first[0] + px as i64 - APRON as i64, first[1] + py as i64 - APRON as i64);
                let span = PAGE_CELLS as i64;
                let (qx, qy) = (x.div_euclid(span), y.div_euclid(span));
                let (source, x, y) = match (qx, qy) {
                    (0, 0) => (&around.own, x, y),
                    _ => match &around.beside[beside_index(qx, qy)] {
                        Some(s) => (s, x - qx * span, y - qy * span),
                        None => (&around.own, x, y),
                    },
                };
                let (page, n) = (&source.page, source.n);
                let (x, y) = (source.origin[0] + x, source.origin[1] + y);
                let (tx, ty) = (x.div_euclid(n as i64) as i32, y.div_euclid(n as i64) as i32);
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
                let (mut material, mut layer) = (pack(tables.base_class, tables.base_rgb), tables.base_layer);
                for (ch, &t) in types.iter().enumerate() {
                    if skip[ch] || t == VOID {
                        continue;
                    }
                    if let Some(m) = tables.of(GROUND_CH_KEYS[ch], t) {
                        (material, layer) = m;
                    }
                }
                self.material[i] = material;
                self.layer_idx[i] = layer;

                // The height every consumer of this page takes, recess and
                // all (elevation.rs's ground_ele). An apron cell off the page
                // has no channels to judge by and takes the raw height.
                let lanes = on_page.then_some(GroundLanes { cells: page.cells });
                let center = |v: i64| (v as f64 + 0.5) / n as f64;
                let ele = ground_ele_at(page.ele, lanes, center(x), center(y)) as i32;
                self.ele[i] = ele as i16;
                // The floor is the chunk's own: an apron column is written
                // from it, and a neighbor's valley must not drop it.
                if inner {
                    min_ele = min_ele.min(ele);
                }
            }
        }
        // One band-aligned floor shared by the whole chunk, under every peak.
        self.floor = (min_ele - TERRAIN_Z_MARGIN).div_euclid(BAND_CELLS as i32) * BAND_CELLS as i32;
        let shown = !skip[ch_index(GroundChannelKey::TRANSPORT)];
        self.read_decks(if shown { Some(&around.own) } else { None }, first)
    }

    /// The decks over the chunk's own columns, from the page they are drawn
    /// from: each column's highest top of each deck, as its slab and pier.
    fn read_decks(&mut self, own: Option<&PageSource>, first: [i64; 2]) -> Result<(), Refused> {
        let mut over = core::mem::take(&mut self.over);
        over.clear();
        let built = collect_decks(&mut over, own, first).and_then(|()| self.spans_of(&mut over));
        self.over = over;
        built
    }

    /// Each column's spans off the ground from the decks over it.
    fn spans_of(&mut self, over: &mut Vec<(usize, usize, Over)>) -> Result<(), Refused> {
        let tables = &*MATERIALS;
        // A deck's highest over a column, where two of its segments meet.
        over.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)).then(b.2.top.total_cmp(&a.2.top)));
        over.dedup_by_key(|o| (o.0, o.1));
        self.spans.clear();
        let mut col = 0;
        let mut i = 0;
        while col < COL_SPAN2 {
            self.span_at[col] = self.spans.len() as u32;
            let from = self.spans.len();
            while i < over.len() && over[i].0 == col {
                let o = over[i].2;
                i += 1;
                let Some((slab, slab_layer)) = tables.of(GroundChannelKey::TRANSPORT, o.kind) else { continue };
                let Some(occupied) = o.occupies(self.ele[col] as i32) else {
                    (self.material[col], self.layer_idx[col]) = (slab, slab_layer);
                    continue;
                };
                let [lo, hi] = occupied.slab;
                try_push(
                    &mut self.spans,
                    Span {
                        lo,
                        hi,
                        material: slab,
                        layer: slab_layer,
                    },
                )?;
                if let (Some([lo, hi]), Some((pier, pier_layer))) = (occupied.pier, tables.of(PIER.0, PIER.1)) {
                    try_push(
                        &mut self.spans,
                        Span {
                            lo,
                            hi,
                            material: pier,
                            layer: pier_layer,
                        },
                    )?;
                }
            }
            // Lowest first, and a higher span keeps what two of them share.
            let column = &mut self.spans[from..];
            column.sort_by_key(|s| (s.lo, s.hi));
            for k in (1..column.len()).rev() {
                column[k - 1].hi = column[k - 1].hi.min(column[k].lo);
            }
            let mut k = from;
            for j in from..self.spans.len() {
                if self.spans[j].hi > self.spans[j].lo {
                    self.spans[k] = self.spans[j];
                    k += 1;
                }
            }
            self.spans.truncate(k);
            col += 1;
        }
        self.span_at[COL_SPAN2] = self.spans.len() as u32;
        Ok(())
    }

    /// Column `i`'s runs, lowest first: its ground from `floor`, then what
    /// stands off it, as `run(lo, hi, material, layer)`, both ends inside.
    pub fn column_runs<E>(&self, i: usize, mut run: impl FnMut(i32, i32, u32, u8) -> Result<(), E>) -> Result<(), E> {
        let top = self.ele[i] as i32 - 1;
        if top >= self.floor {
            run(self.floor, top, self.material[i], self.layer_idx[i])?;
        }
        for s in &self.spans[self.span_at[i] as usize..self.span_at[i + 1] as usize] {
            run(s.lo, s.hi - 1, s.material, s.layer)?;
        }
        Ok(())
    }

    /// Paints the chunk's own columns into `volume`, floor to top and what
    /// stands off it. The apron's are not painted: the volume is the
    /// chunk's, and they exist only as columns of the run set.
    pub fn paint(&self, volumes: &mut Volumes, volume: u32) -> bool {
        let mut painted = true;
        for cy in 0..CHUNK_CELLS {
            for cx in 0..CHUNK_CELLS {
                let _ = self.column_runs(apron_col(cx, cy) as usize, |lo, hi, material, layer_idx| {
                    painted &= volumes.fill_column(volume, (cx, cy), lo, hi, Voxel { material, layer_idx, sub: 0 });
                    Ok::<(), ()>(())
                });
                if !painted {
                    return false;
                }
            }
        }
        true
    }
}
