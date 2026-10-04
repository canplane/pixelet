// pxl-render/mesh/paint/terrain/adapter.rs

//! A terrain page's chunk as the render pipeline reads it: per cell one
//! material (the last non-void channel wins) and one top, painted as a column
//! from a floor shared by the chunk. Decks stand off the column as runs of
//! their own; a bore or a cutting takes the air over its floor out of the
//! column.
//! The floor is rounded down to a whole band, since a volume allocates whole
//! bands and a floor inside one would open another for a few voxels.

use pxl_spec::bake::constant::{BAND_CELLS, U_PER_CELL};
use pxl_spec::bake::deck::{Decks, Occupies, Over};
use pxl_spec::bake::format::{ch_index, GroundChannelKey, GROUND_CH_KEYS};
use pxl_spec::bake::ground::{shown_ground, ShownGround, BASE};
use pxl_spec::bake::pack::{unpack, GroundCell};
use pxl_spec::cell::cube::cube_types;
use pxl_spec::procedural::{pier_stands, Frame};
use pxl_spec::space::adapter::epsg3857::u_per_m;
use pxl_spec::space::hierarchy::{ancestor_of, offset_in};
use pxl_spec::space::pyramid::{CHUNK_CELLS, GROUND, PAGE_CELLS};

use crate::mesh::paint::palette::shader_meta::{rgb255, GROUND_LAYER_META};
use crate::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use crate::mesh::volume::{pack_material, MaterialClasses, Volumes, Voxel, NO_LAYER};
use crate::mesh::voxel_scan::{apron_col, APRON, COL_SPAN};
use crate::mesh::{try_push, Refused};
use std::sync::LazyLock;

const TERRAIN_Z_MARGIN: i32 = 4;
const COL_SPAN2: usize = (COL_SPAN * COL_SPAN) as usize;

/// In `GROUND_CH_KEYS`' order, which is the paint order: a later non-void type wins.
pub const CHANNELS: usize = GROUND_CH_KEYS.len();

const VOID: u8 = 0;

/// Flat arrays rather than a map: the per-cell decode was dominated by map
/// lookups. `has` is separate because [0,0,0] and layer 0 are real.
#[derive(Clone, Copy)]
pub struct Channel {
    pub rgb: [[u8; 3]; 256],
    pub layer: [u8; 256],
    pub has: [bool; 256],
    pub class: u8,
}

impl Default for Channel {
    fn default() -> Self {
        Channel {
            rgb: [[0; 3]; 256],
            layer: [0; 256],
            has: [false; 256],
            class: 0,
        }
    }
}

pub struct Materials {
    pub channels: [Channel; CHANNELS],
    pub base_rgb: [u8; 3],
    pub base_layer: u8,
    pub base_class: u8,
}

pub static MATERIALS: LazyLock<Materials> = LazyLock::new(Materials::of_palette);

fn class_of(ch: GroundChannelKey) -> u8 {
    match ch {
        GroundChannelKey::LAND => MaterialClasses::LAND,
        GroundChannelKey::WATER => MaterialClasses::WATER,
        GroundChannelKey::STRUCTURE => MaterialClasses::STRUCTURE,
        GroundChannelKey::TRANSPORT => MaterialClasses::TRANSPORT,
    }
}

impl Materials {
    /// Packed `0xCC_RRGGBB`; none for a type the palette gives no color.
    pub fn of(&self, ch: GroundChannelKey, t: u8) -> Option<(u32, u8)> {
        let c = &self.channels[ch_index(ch)];
        let t = t as usize;
        c.has[t].then(|| (pack_material(c.class, c.rgb[t]), c.layer[t]))
    }

    fn of_palette() -> Materials {
        let mut channels = [Channel::default(); CHANNELS];
        for (dst, ch) in channels.iter_mut().zip(GROUND_CH_KEYS) {
            dst.class = class_of(ch);
        }
        for m in GROUND_LAYER_META.iter() {
            let (dst, t) = (&mut channels[ch_index(m.ch_key)], m.type_id as usize);
            (dst.rgb[t], dst.layer[t], dst.has[t]) = (rgb255(m.rgb), m.layer_index as u8, true);
        }
        let base = &channels[ch_index(BASE.channel)];
        let t = BASE.type_id as usize;
        let (base_rgb, base_layer) = if base.has[t] { (base.rgb[t], base.layer[t]) } else { ([0; 3], NO_LAYER as u8) };
        Materials {
            channels,
            base_rgb,
            base_layer,
            base_class: class_of(BASE.channel),
        }
    }
}

/// The decks over the columns `s` is read for, the apron's too: a column the
/// apron reads without its deck stands at another height than the chunk beside
/// draws it, and the faces either side are shaded for a step that is not there.
/// `s` is the page at `quadrant` beside the chunk's own and is read for the
/// columns `reads` says; `corner` is the apron's first column in the own page's cells.
fn collect_decks(over: &mut Vec<(usize, usize, Over)>, s: &PageSource, corner: [i64; 2], quadrant: [i64; 2], reads: impl Fn([i64; 2]) -> bool) -> Result<(), Refused> {
    let n = f64::from(s.n);
    let origin = [0, 1].map(|k| (s.origin[k] + corner[k] - quadrant[k] * PAGE_CELLS as i64) as f64 / n);
    let side = COL_SPAN as i64;
    let mut grown = Ok(());
    s.page.decks.over_grid(Some(s.frame), origin, 1.0 / n, [side, side], |cx, cy, d, o| {
        if grown.is_ok() && reads(ancestor_of([corner[0] + cx, corner[1] + cy], GROUND.page_cell_shamt())) {
            grown = try_push(over, ((cy * side + cx) as usize, d, o));
        }
    });
    grown
}

/// A building's concrete: nothing in the catalog is a pier's own.
const PIER: (GroundChannelKey, u8) = (GroundChannelKey::STRUCTURE, cube_types::structure::building);

#[derive(Clone, Copy)]
pub struct PageView<'a> {
    pub cells: &'a [GroundCell],
    pub ele: &'a [i16],
    pub decks: &'a Decks,
}

/// The page drawn in place of this one (its own, or a coarser one whose cell
/// stands for `n` × `n` of its cells until its own comes).
#[derive(Clone, Copy)]
pub struct PageSource<'a> {
    pub page: PageView<'a>,
    /// Where the page lies, for its piers' stations.
    pub frame: Frame,
    pub origin: [i64; 2],
    pub n: u32,
}

pub struct Around<'a> {
    pub own: PageSource<'a>,
    /// None where the apron does not reach or nothing is drawn; those cells are
    /// read from `own`, whose elevation is padded past its edge.
    pub beside: [Option<PageSource<'a>>; 8],
}

pub const fn beside_index(qx: i64, qy: i64) -> usize {
    let k = (qx + 1) + 3 * (qy + 1);
    (if k > 4 { k - 1 } else { k }) as usize
}

/// Apron cells are read from what their own page is drawn from, as the chunk
/// beside draws it, so the face mask sees what is on screen; they are never painted or emitted.
pub struct ChunkSample {
    /// Packed 0xCC_RRGGBB.
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    /// In cells; a column's top is one below it.
    pub ele: Vec<i16>,
    pub floor: i32,
    /// Column `i`'s spans are `spans[span_at[i]..span_at[i + 1]]`, lowest first,
    /// none overlapping; the apron's have none.
    pub span_at: Vec<u32>,
    pub spans: Vec<Span>,
    /// `[lo, hi)`, empty where none does.
    pub bore: Vec<Bore>,
    /// Each column's shown ground, nothing hidden: where a pier stands is the data's.
    shown: Vec<ShownGround>,
    over: Vec<(usize, usize, Over)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bore {
    pub air: [i32; 2],
    pub material: u32,
    pub layer: u8,
}

/// `lo` up to `hi` exclusive, in cells.
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
            bore: vec![Bore::default(); COL_SPAN2],
            shown: vec![BASE; COL_SPAN2],
            over: Vec::new(),
        }
    }
}

impl ChunkSample {
    /// Each cell takes the material of the source cell it lies in and the source
    /// surface's height at its own center, so a coarser page invents nothing.
    pub fn read(&mut self, around: &Around, first: [i64; 2]) -> Result<(), Refused> {
        let tables = &*MATERIALS;
        let mut min_ele = i32::MAX;
        let last = PAGE_CELLS as i32 - 1;
        for py in 0..COL_SPAN {
            for px in 0..COL_SPAN {
                let i = (py * COL_SPAN + px) as usize;
                let own = APRON..COL_SPAN - APRON;
                let inner = own.contains(&px) && own.contains(&py);
                let (x, y) = (first[0] + px as i64 - APRON as i64, first[1] + py as i64 - APRON as i64);
                let [qx, qy] = ancestor_of([x, y], GROUND.page_cell_shamt());
                let (source, [x, y]) = match (qx, qy) {
                    (0, 0) => (&around.own, [x, y]),
                    _ => match &around.beside[beside_index(qx, qy)] {
                        Some(s) => (s, offset_in([x, y], GROUND.page_cell_shamt())),
                        None => (&around.own, [x, y]),
                    },
                };
                let (page, n) = (&source.page, source.n);
                let (x, y) = (source.origin[0] + x, source.origin[1] + y);
                let (tx, ty) = (x.div_euclid(n as i64) as i32, y.div_euclid(n as i64) as i32);
                // Only the elevation is padded past the page. An apron cell off it keeps its
                // height, all the face mask reads, and a base material nothing draws.
                let on_page = (0..=last).contains(&tx) && (0..=last).contains(&ty);
                let types = if on_page {
                    unpack(page.cells[GROUND.cell_index(tx as u32, ty as u32) as usize])
                } else {
                    [VOID; 4]
                };
                self.shown[i] = shown_ground(|k| u32::from(types[ch_index(k)]));

                let (mut material, mut layer) = (pack_material(tables.base_class, tables.base_rgb), tables.base_layer);
                for (ch, &t) in types.iter().enumerate() {
                    if t == VOID {
                        continue;
                    }
                    if let Some(m) = tables.of(GROUND_CH_KEYS[ch], t) {
                        (material, layer) = m;
                    }
                }
                self.material[i] = material;
                self.layer_idx[i] = layer;

                // An apron cell off the page has no channels and takes the raw height.
                let lanes = on_page.then_some(GroundLanes { cells: page.cells });
                let center = |v: i64| (v as f64 + 0.5) / n as f64;
                let ele = ground_ele_at(page.ele, lanes, center(x), center(y)) as i32;
                self.ele[i] = ele as i16;
                // The floor is the chunk's own: a neighbor's valley must not drop it.
                if inner {
                    min_ele = min_ele.min(ele);
                }
            }
        }
        self.read_decks(around, first)?;
        let bored = self.bore.iter().filter(|b| b.air[0] < b.air[1]).map(|b| b.air[0] - 1).min().unwrap_or(i32::MAX);
        self.floor = (min_ele.min(bored) - TERRAIN_Z_MARGIN).div_euclid(BAND_CELLS as i32) * BAND_CELLS as i32;
        Ok(())
    }

    /// Each column's decks from the page its height is read from (`read`).
    fn read_decks(&mut self, around: &Around, first: [i64; 2]) -> Result<(), Refused> {
        let mut over = core::mem::take(&mut self.over);
        over.clear();
        let cell_per_m = u_per_m(around.own.frame.corner[1]) / U_PER_CELL;
        let corner = first.map(|v| v - APRON as i64);
        // A column with no page beside is read from the chunk's own, as its height is.
        let beside = |[qx, qy]: [i64; 2]| if (qx, qy) == (0, 0) { None } else { around.beside[beside_index(qx, qy)].as_ref() };
        let mut built = collect_decks(&mut over, &around.own, corner, [0, 0], |q| beside(q).is_none());
        for qy in -1..=1 {
            for qx in -1..=1 {
                if let (Some(s), true) = (beside([qx, qy]), built.is_ok()) {
                    built = collect_decks(&mut over, s, corner, [qx, qy], |q| q == [qx, qy]);
                }
            }
        }
        let built = built.and_then(|()| self.spans_of(&mut over, cell_per_m));
        self.over = over;
        built
    }

    fn spans_of(&mut self, over: &mut Vec<(usize, usize, Over)>, cell_per_m: f64) -> Result<(), Refused> {
        let tables = &*MATERIALS;
        // A line over a column is all its segments sweep there; a station is one segment's,
        // so the pier is either's.
        over.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        over.dedup_by(|b, a| {
            let same = (a.0, a.1) == (b.0, b.1);
            if same {
                (a.2.low, a.2.top, a.2.pier) = (a.2.low.min(b.2.low), a.2.top.max(b.2.top), a.2.pier || b.2.pier);
            }
            same
        });
        self.spans.clear();
        let mut col = 0;
        let mut i = 0;
        while col < COL_SPAN2 {
            self.span_at[col] = self.spans.len() as u32;
            self.bore[col] = Bore::default();
            let from = self.spans.len();
            let col_from = i;
            while i < over.len() && over[i].0 == col {
                let o = over[i].2;
                i += 1;
                let Some((slab, slab_layer)) = tables.of(GroundChannelKey::TRANSPORT, o.kind) else { continue };
                let ground = self.ele[col] as i32;
                let (slab_at, pier_at) = match o.occupies(ground, cell_per_m) {
                    Occupies::Grade => {
                        (self.material[col], self.layer_idx[col]) = (slab, slab_layer);
                        continue;
                    }
                    Occupies::Through { air } => {
                        if air[1] - air[0] > self.bore[col].air[1] - self.bore[col].air[0] {
                            self.bore[col] = Bore {
                                air,
                                material: slab,
                                layer: slab_layer,
                            };
                        }
                        continue;
                    }
                    Occupies::Above { slab, pier } => (slab, pier),
                };
                let [lo, hi] = slab_at;
                try_push(
                    &mut self.spans,
                    Span {
                        lo,
                        hi,
                        material: slab,
                        layer: slab_layer,
                    },
                )?;
                let column = over[col_from..].iter().take_while(|c| c.0 == col).map(|c| c.2);
                if let (Some([lo, hi]), Some((pier, pier_layer))) = (pier_at.and_then(|p| pier_stands(p, self.shown[col], column, ground, cell_per_m)), tables.of(PIER.0, PIER.1)) {
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
            // Lowest first; a higher span keeps what two share.
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

    /// The column's ground as inclusive runs: a top face at `ele` is cells up to
    /// `ele − 1` (`hi − 1` below). Through a bore: the ground under its floor,
    /// the way on the floor's cell, and the ground standing over its air.
    pub fn ground_runs<E>(&self, i: usize, from: i32, to: i32, mut run: impl FnMut(i32, i32, u32, u8) -> Result<(), E>) -> Result<(), E> {
        let top = (self.ele[i] as i32).min(to);
        let (m, l) = (self.material[i], self.layer_idx[i]);
        let mut part = |lo: i32, hi: i32, m: u32, l: u8| if lo.max(from) < hi { run(lo.max(from), hi - 1, m, l) } else { Ok(()) };
        let b = self.bore[i];
        if b.air[0] >= b.air[1] {
            return part(from, top, m, l);
        }
        let floor = b.air[0];
        part(from, (floor - 1).min(top), m, l)?;
        part(floor - 1, floor.min(top), b.material, b.layer)?;
        part(b.air[1], top, m, l)
    }

    pub fn column_runs<E>(&self, i: usize, mut run: impl FnMut(i32, i32, u32, u8) -> Result<(), E>) -> Result<(), E> {
        self.ground_runs(i, self.floor, i32::MAX, &mut run)?;
        for s in &self.spans[self.span_at[i] as usize..self.span_at[i + 1] as usize] {
            run(s.lo, s.hi - 1, s.material, s.layer)?;
        }
        Ok(())
    }

    /// The apron's columns are not painted: the volume is the chunk's.
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
