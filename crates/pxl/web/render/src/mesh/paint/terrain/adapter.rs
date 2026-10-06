// pxl-render/mesh/paint/terrain/adapter.rs

//! A terrain tile's chunk as the render pipeline reads it: per cell one
//! material, its look's, and one top, painted as a column
//! from a floor shared by the chunk. Decks stand off the column as runs of
//! their own; a bore or a cutting takes the air over its floor out of the
//! column.
//! The floor is rounded down to a whole band, since a volume allocates whole
//! bands and a floor inside one would open another for a few voxels.

use pxl_spec::bake::constant::{BAND_CELLS, U_PER_CELL};
use pxl_spec::bake::deck::{Decks, Occupies, Over};
use pxl_spec::bake::fold::majority;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::cell::color;
use pxl_spec::cell::cube::cube_types::{ground, sentinel, Ground};
use pxl_spec::cell::cube::Kind;
use pxl_spec::cell::object::PlaceableCube;
use pxl_spec::procedural::{pier_stands, Frame};
use pxl_spec::space::adapter::epsg3857::u_per_m;
use pxl_spec::space::hierarchy::{ancestor_of, offset_in};
use pxl_spec::space::pyramid::GROUND;
use pxl_spec::space::zoom::Zoom;
use xpute_core::math::scalar::round;

use crate::look::color::mean_srgb;
use crate::mesh::paint::palette::shader_meta::{layer_mean, rgb255};
use crate::mesh::paint::palette::tex_array::LAYER_IDX;
use crate::mesh::paint::terrain::elevation::{ground_ele_at, GroundLanes};
use crate::mesh::run::{APRON, COL_SPAN};
use crate::mesh::word::{pack_material, unpack_material, MaterialClasses, NO_LAYER};
use crate::mesh::{try_push, Refused};
use std::sync::LazyLock;

const TERRAIN_Z_MARGIN: i32 = 4;
const COL_SPAN2: usize = (COL_SPAN * COL_SPAN) as usize;

/// Each byte's material word and layer: a look's, or unknown's for a byte
/// that is none.
pub struct Materials {
    looks: [Option<(u32, u8)>; 256],
    unknown: (u32, u8),
}

pub static MATERIALS: LazyLock<Materials> = LazyLock::new(Materials::of_palette);

fn class_of(kind: Kind) -> u8 {
    match kind {
        Kind::Land => MaterialClasses::LAND,
        Kind::Water => MaterialClasses::WATER,
        Kind::Structure => MaterialClasses::STRUCTURE,
        Kind::Transport => MaterialClasses::TRANSPORT,
    }
}

impl Materials {
    /// Packed `0xCC_RRGGBB`; none for a byte that is no look.
    pub fn of(&self, cell: GroundCell) -> Option<(u32, u8)> {
        self.looks[cell as usize]
    }

    /// What a cell draws as: a byte that is no look as unknown.
    pub fn drawn(&self, cell: GroundCell) -> (u32, u8) {
        self.of(cell).unwrap_or(self.unknown)
    }

    /// What cells of several looks draw as together, one drawn cell over them:
    /// one look keeps its material and layer; several, the mean of what each
    /// shows, the pack's mean where it is drawn, in the class of `of`'s.
    pub fn mean(&self, of: GroundCell, cells: &[GroundCell], textured: bool) -> (u32, u8) {
        if cells.iter().all(|&c| c == of) {
            return self.drawn(of);
        }
        let shown = |c: GroundCell| {
            let (m, l) = self.drawn(c);
            let rgb = unpack_material(m).1;
            if textured {
                layer_mean(u32::from(l), rgb)
            } else {
                rgb
            }
        };
        let rgb = mean_srgb(cells.iter().filter(|&&c| c != sentinel::void).map(|&c| shown(c))).unwrap_or_else(|| shown(of));
        (pack_material(unpack_material(self.drawn(of).0).0, rgb), NO_LAYER as u8)
    }

    fn of_palette() -> Materials {
        let mut looks = [None; 256];
        for g in Ground::ALL {
            let rgb = color::ground(g as u8).map_or([0; 3], |s| rgb255(s.rgb));
            looks[g as usize] = Some((pack_material(class_of(g.kind()), rgb), LAYER_IDX(PlaceableCube::Ground(g)) as u8));
        }
        let rgb = color::sentinel(sentinel::unknown).map_or([0; 3], |s| rgb255(s.rgb));
        Materials {
            looks,
            unknown: (pack_material(MaterialClasses::LAND, rgb), NO_LAYER as u8),
        }
    }
}

/// The decks over the columns `s` is read for, the apron's too: a column the
/// apron reads without its deck stands at another height than the chunk beside
/// draws it, and the faces either side are shaded for a step that is not there.
/// `s` is the tile at `quadrant` beside the chunk's own and is read for the
/// columns `reads` says; `corner` is the apron's first column in the own tile's
/// drawn cells, a tile `1 << tile_shamt` of them across.
fn collect_decks(over: &mut Vec<(usize, usize, Over)>, s: &TileSource, corner: [i64; 2], quadrant: [i64; 2], tile_shamt: u32, reads: impl Fn([i64; 2]) -> bool) -> Result<(), Refused> {
    let per = s.per_drawn();
    let origin = [0, 1].map(|k| (s.origin[k] + corner[k] - (quadrant[k] << tile_shamt)) as f64 * per);
    let side = COL_SPAN as i64;
    let mut grown = Ok(());
    s.tile.decks.over_grid(Some(s.frame), origin, per, [side, side], |cx, cy, d, o| {
        if grown.is_ok() && reads(ancestor_of([corner[0] + cx, corner[1] + cy], tile_shamt)) {
            grown = try_push(over, ((cy * side + cx) as usize, d, o));
        }
    });
    grown
}

/// A building's concrete: nothing in the catalog is a pier's own.
const PIER: GroundCell = ground::building;

#[derive(Clone, Copy)]
pub struct TileView<'a> {
    pub cells: &'a [GroundCell],
    pub ele: &'a [i16],
    pub decks: &'a Decks,
}

/// The tile drawn in place of this one: its own, or a coarser one until its own comes.
#[derive(Clone, Copy)]
pub struct TileSource<'a> {
    pub tile: TileView<'a>,
    /// Where the tile lies, for its piers' stations.
    pub frame: Frame,
    /// The chunk's tile's first cell in this one, in drawn cells.
    pub origin: [i64; 2],
    /// log2 of its cell in drawn cells: a coarser tile's stands for several,
    /// and at −1 four of the tile's make one drawn cell.
    pub shift: i32,
}

impl TileSource<'_> {
    /// Its cells a drawn cell spans, along an axis.
    fn per_drawn(&self) -> f64 {
        libm::exp2(-f64::from(self.shift))
    }
}

pub struct Around<'a> {
    pub own: TileSource<'a>,
    /// None where the apron does not reach or nothing is drawn; those cells are
    /// read from `own`, whose elevation is padded past its edge.
    pub beside: [Option<TileSource<'a>>; 8],
    /// log2 of a tile's side in drawn cells.
    pub tile_shamt: u32,
    /// The drawn cells', whose parity breaks a tie among four looks as a fold's does.
    pub zoom: Zoom,
    /// Whether the pack is drawn: four looks' mean is of what each shows.
    pub textured: bool,
}

pub const fn beside_index(qx: i64, qy: i64) -> usize {
    let k = (qx + 1) + 3 * (qy + 1);
    (if k > 4 { k - 1 } else { k }) as usize
}

/// Apron cells are read from what their own tile is drawn from, as the chunk
/// beside draws it, so the face mask sees what is on screen; they are never painted or emitted.
pub struct ChunkSample {
    /// Packed 0xCC_RRGGBB.
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    /// In cells; a column's top is one below it.
    pub ele: Vec<i32>,
    pub floor: i32,
    /// Column `i`'s spans are `spans[span_at[i]..span_at[i + 1]]`, lowest first,
    /// none overlapping; the apron's have none.
    pub span_at: Vec<u32>,
    pub spans: Vec<Span>,
    /// `[lo, hi)`, empty where none does.
    pub bore: Vec<Bore>,
    /// Each column's look: where a pier stands is the data's, and a void one has no ground.
    look: Vec<GroundCell>,
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
            look: vec![ground::BASE; COL_SPAN2],
            over: Vec::new(),
        }
    }
}

impl ChunkSample {
    /// Each drawn cell takes the material of the source cell it lies in and the
    /// source surface's height at its own center, so a coarser tile invents
    /// nothing; over four of a finer tile's, their majority look, their mean
    /// height and the mean of what they show, as a retina's pixel is its
    /// sensors'.
    pub fn read(&mut self, around: &Around, first: [i64; 2]) -> Result<(), Refused> {
        let mut min_ele = i32::MAX;
        let shamt = around.tile_shamt;
        for py in 0..COL_SPAN {
            for px in 0..COL_SPAN {
                let i = (py * COL_SPAN + px) as usize;
                let own = APRON..COL_SPAN - APRON;
                let inner = own.contains(&px) && own.contains(&py);
                let (x, y) = (first[0] + px as i64 - APRON as i64, first[1] + py as i64 - APRON as i64);
                let [qx, qy] = ancestor_of([x, y], shamt);
                let (source, [x, y]) = match (qx, qy) {
                    (0, 0) => (&around.own, [x, y]),
                    _ => match &around.beside[beside_index(qx, qy)] {
                        Some(s) => (s, offset_in([x, y], shamt)),
                        None => (&around.own, [x, y]),
                    },
                };
                let (look, ele) = Self::cell_of(source, [source.origin[0] + x, source.origin[1] + y], around);
                self.look[i] = look.0;
                (self.material[i], self.layer_idx[i]) = look.1;
                self.ele[i] = ele;
                // The floor is the chunk's own: a neighbor's valley must not drop it.
                if inner && look.0 != sentinel::void {
                    min_ele = min_ele.min(ele);
                }
            }
        }
        self.read_decks(around, first)?;
        let bored = self.bore.iter().filter(|b| b.air[0] < b.air[1]).map(|b| b.air[0] - 1).min().unwrap_or(i32::MAX);
        // A chunk with no surface has no floor: nothing of it is drawn from one.
        self.floor = match min_ele.min(bored) {
            i32::MAX => i32::MIN,
            lowest => (lowest - TERRAIN_Z_MARGIN).div_euclid(BAND_CELLS as i32) * BAND_CELLS as i32,
        };
        Ok(())
    }

    /// The drawn cell at `[x, y]` drawn cells into `s`: its look, material and height.
    fn cell_of(s: &TileSource, [x, y]: [i64; 2], around: &Around) -> ((GroundCell, (u32, u8)), i32) {
        let tables = &*MATERIALS;
        let tile = &s.tile;
        let last = GROUND.tile_cells() as i64 - 1;
        // Only the elevation is padded past the tile. A cell off it keeps its height,
        // all the face mask reads, and the base look nothing draws.
        let at = |tx: i64, ty: i64| {
            let on_tile = (0..=last).contains(&tx) && (0..=last).contains(&ty);
            let cell = if on_tile { tile.cells[GROUND.cell_index(tx as u32, ty as u32) as usize] } else { ground::BASE };
            (cell, on_tile.then_some(GroundLanes { cells: tile.cells }))
        };
        if s.shift >= 0 {
            let n = 1i64 << s.shift;
            let (cell, lanes) = at(x.div_euclid(n), y.div_euclid(n));
            let center = |v: i64| (v as f64 + 0.5) / n as f64;
            return ((cell, tables.drawn(cell)), ground_ele_at(tile.ele, lanes, center(x), center(y)) as i32);
        }
        xpute_core::ensure!(s.shift == -1, ENOTRECOVERABLE, s.shift);
        let four: [(GroundCell, Option<GroundLanes>, [i64; 2]); 4] = core::array::from_fn(|q| {
            let (tx, ty) = (2 * x + (q & 1) as i64, 2 * y + (q >> 1) as i64);
            let (cell, lanes) = at(tx, ty);
            (cell, lanes, [tx, ty])
        });
        let looks = four.map(|f| f.0);
        let look = majority(looks, around.zoom);
        // The mean of the heights that have a surface, or of all where none does.
        let heights = four.map(|(_, lanes, [tx, ty])| ground_ele_at(tile.ele, lanes, tx as f64 + 0.5, ty as f64 + 0.5) as f64);
        let any = looks.iter().any(|&c| c != sentinel::void);
        let (sum, n) = (0..4).filter(|&q| !any || looks[q] != sentinel::void).fold((0.0, 0.0), |(s, n), q| (s + heights[q], n + 1.0));
        ((look, tables.mean(look, &looks, around.textured)), round(sum / n) as i32)
    }

    /// Each column's decks from the tile its height is read from (`read`).
    fn read_decks(&mut self, around: &Around, first: [i64; 2]) -> Result<(), Refused> {
        let mut over = core::mem::take(&mut self.over);
        over.clear();
        let cell_per_m = u_per_m(around.own.frame.corner[1]) / U_PER_CELL;
        let corner = first.map(|v| v - APRON as i64);
        // A column with no tile beside is read from the chunk's own, as its height is.
        let beside = |[qx, qy]: [i64; 2]| if (qx, qy) == (0, 0) { None } else { around.beside[beside_index(qx, qy)].as_ref() };
        let shamt = around.tile_shamt;
        let mut built = collect_decks(&mut over, &around.own, corner, [0, 0], shamt, |q| beside(q).is_none());
        for qy in -1..=1 {
            for qx in -1..=1 {
                if let (Some(s), true) = (beside([qx, qy]), built.is_ok()) {
                    built = collect_decks(&mut over, s, corner, [qx, qy], shamt, |q| q == [qx, qy]);
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
        over.sort_by_key(|a| (a.0, a.1));
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
                let Some((slab, slab_layer)) = tables.of(o.kind) else { continue };
                let ground = self.ele[col];
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
                if let (Some([lo, hi]), Some((pier, pier_layer))) = (pier_at.and_then(|p| pier_stands(p, self.look[col], column, ground, cell_per_m)), tables.of(PIER)) {
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

    /// The sample of a chunk `k` zooms finer than the leaf chunk `leaf` was
    /// read for, its first cell `at` its own cells into that chunk: each column
    /// is the leaf column it lies in, heights in its own cells. Terrain is
    /// drawn as finely as it is held, so nothing is invented.
    pub fn refine(&mut self, leaf: &ChunkSample, k: u32, at: [u32; 2]) -> Result<(), Refused> {
        let scale = 1i32 << k;
        let in_leaf = |v: u32, p: u32| (i64::from(v) + i64::from(p) - i64::from(APRON)).div_euclid(i64::from(scale)) + i64::from(APRON);
        self.spans.clear();
        for py in 0..COL_SPAN {
            for px in 0..COL_SPAN {
                let i = (py * COL_SPAN + px) as usize;
                let j = (in_leaf(at[1], py) * i64::from(COL_SPAN) + in_leaf(at[0], px)) as usize;
                self.material[i] = leaf.material[j];
                self.layer_idx[i] = leaf.layer_idx[j];
                self.look[i] = leaf.look[j];
                self.ele[i] = leaf.ele[j] * scale;
                let b = leaf.bore[j];
                self.bore[i] = Bore { air: b.air.map(|v| v * scale), ..b };
                self.span_at[i] = self.spans.len() as u32;
                for s in &leaf.spans[leaf.span_at[j] as usize..leaf.span_at[j + 1] as usize] {
                    try_push(
                        &mut self.spans,
                        Span {
                            lo: s.lo * scale,
                            hi: s.hi * scale,
                            ..*s
                        },
                    )?;
                }
            }
        }
        self.span_at[COL_SPAN2] = self.spans.len() as u32;
        self.floor = if leaf.floor == i32::MIN { i32::MIN } else { leaf.floor * scale };
        Ok(())
    }

    /// The column's ground as inclusive runs: a top face at `ele` is cells up to
    /// `ele − 1` (`hi − 1` below). Through a bore: the ground under its floor,
    /// the way on the floor's cell, and the ground standing over its air.
    /// Whether column `i` has ground: a void cell's terrain gives no surface.
    pub fn has_ground(&self, i: usize) -> bool {
        self.look[i] != sentinel::void
    }

    pub fn ground_runs<E>(&self, i: usize, from: i32, to: i32, mut run: impl FnMut(i32, i32, u32, u8) -> Result<(), E>) -> Result<(), E> {
        if !self.has_ground(i) {
            return Ok(());
        }
        let top = self.ele[i].min(to);
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
}

#[cfg(test)]
#[path = "adapter.test.rs"]
mod test;
