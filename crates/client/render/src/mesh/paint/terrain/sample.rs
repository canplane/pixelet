// pxl-render/mesh/paint/terrain/sample.rs

//! A terrain tile's chunk as the render pipeline reads it: per cell one
//! material, its look's, and one top, painted as a column
//! from a floor shared by the chunk. Bridges and tunnels stand off the column as runs of
//! their own; a bore or a cutting takes the air over its floor out of the
//! column.

use pxl_spec::bake::format::GroundCell;
use pxl_spec::bake::separation::{Crossing, Occupies, Separation};
use pxl_spec::block::color;
use pxl_spec::block::cubes::{ground, sentinel, Ground};
use pxl_spec::block::object::Cube;
use pxl_spec::procedural::{pier_stands, Frame};
use pxl_spec::space::coord::adapter::epsg3857::u_per_m;
use pxl_spec::space::hierarchy::tree::{ancestor_of, offset_in};
use pxl_spec::space::layer::lattice::{GROUND, TILE_CELLS};

use crate::mesh::paint::palette::rgb255;
use crate::mesh::paint::palette::LAYER_IDX;
use crate::mesh::paint::terrain::elevation::ground_ele_at;
use crate::mesh::run::{APRON, COL_SPAN};
use crate::mesh::word::{pack_material, Side, NO_LAYER};
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

impl Materials {
    /// Packed `0xSS_RRGGBB`; none for a byte that is no look.
    pub fn of(&self, cell: GroundCell) -> Option<(u32, u8)> {
        self.looks[cell as usize]
    }

    /// What a cell draws as: a byte that is no look as unknown.
    pub fn drawn(&self, cell: GroundCell) -> (u32, u8) {
        self.of(cell).unwrap_or(self.unknown)
    }

    fn of_palette() -> Materials {
        let mut looks = [None; 256];
        for g in Ground::ALL {
            let rgb = color::ground(g as u8).map_or([0; 3], |s| rgb255(s.rgb));
            looks[g as usize] = Some((pack_material(Side::BASE, rgb), LAYER_IDX(Cube::Ground(g)) as u8));
        }
        let rgb = color::sentinel(sentinel::unknown).map_or([0; 3], |s| rgb255(s.rgb));
        Materials {
            looks,
            unknown: (pack_material(Side::BASE, rgb), NO_LAYER as u8),
        }
    }
}

/// The lines over the columns `s` is read for, the apron's too: a column the
/// apron reads without its lines stands at another height than the chunk beside
/// draws it, and the faces either side are shaded for a step that is not there.
/// `s` is the tile at `quadrant` beside the chunk's own and is read for the
/// columns `reads` says; `corner` is the apron's first column in the own tile's cells.
fn collect_separation(crossing: &mut Vec<(usize, usize, Crossing)>, s: &TileSource, corner: [i64; 2], quadrant: [i64; 2], span: u32, reads: impl Fn([i64; 2]) -> bool) -> Result<(), Refused> {
    let n = f64::from(s.n);
    let origin = [0, 1].map(|k| (s.origin[k] + corner[k] - quadrant[k] * TILE_CELLS as i64) as f64 / n);
    let side = i64::from(span);
    let mut grown = Ok(());
    s.tile.separation.crossing_grid(Some(s.frame), origin, 1.0 / n, [side, side], |cx, cy, d, o| {
        if grown.is_ok() && reads(ancestor_of([corner[0] + cx, corner[1] + cy], GROUND.tile_cell_shamt())) {
            grown = try_push(crossing, ((cy * side + cx) as usize, d, o));
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
    pub separation: &'a Separation,
}

/// The tile drawn in place of this one (its own, or a coarser one whose cell
/// stands for `n` × `n` of its cells until its own comes).
#[derive(Clone, Copy)]
pub struct TileSource<'a> {
    pub tile: TileView<'a>,
    /// Where the tile lies, for its piers' stations.
    pub frame: Frame,
    pub origin: [i64; 2],
    pub n: u32,
}

impl TileSource<'_> {
    /// A cell's look and height as a chunk draws it: the look of the source
    /// cell it lies in, the source surface's height at its own center, so a
    /// coarser tile invents nothing. `at` is in the drawn tile's cells.
    pub fn cell_at(&self, at: [i64; 2]) -> (GroundCell, i32) {
        let last = TILE_CELLS as i32 - 1;
        let (tile, n) = (&self.tile, self.n);
        let (x, y) = (self.origin[0] + at[0], self.origin[1] + at[1]);
        let (tx, ty) = (x.div_euclid(n as i64) as i32, y.div_euclid(n as i64) as i32);
        // Only the elevation is padded past the tile. An apron cell off it keeps its
        // height, all the face mask reads, and the base look nothing draws.
        let on_tile = (0..=last).contains(&tx) && (0..=last).contains(&ty);
        let cell = if on_tile { tile.cells[GROUND.cell_index(tx as u32, ty as u32) as usize] } else { ground::BASE };
        let center = |v: i64| (v as f64 + 0.5) / n as f64;
        (cell, ground_ele_at(tile.ele, center(x), center(y)) as i32)
    }
}

pub struct Around<'a> {
    pub own: TileSource<'a>,
    /// None where the apron does not reach or nothing is drawn; those cells are
    /// read from `own`, whose elevation is padded past its edge.
    pub beside: [Option<TileSource<'a>>; 8],
}

pub const fn beside_index(qx: i64, qy: i64) -> usize {
    let k = (qx + 1) + 3 * (qy + 1);
    (if k > 4 { k - 1 } else { k }) as usize
}

/// Apron cells are read from what their own tile is drawn from, as the chunk
/// beside draws it, so the face mask sees what is on screen; they are never painted or emitted.
pub struct ChunkSample {
    /// Packed 0xSS_RRGGBB.
    pub material: Vec<u32>,
    pub layer_idx: Vec<u8>,
    /// In cells; a column's top is one below it.
    pub ele: Vec<i32>,
    pub floor: i32,
    /// Columns a row, its own and the apron's (`run.rs` `apron_col`).
    pub span: u32,
    /// Its cells a leaf cell's side: past the leaves, a way's slab stays a leaf cell thick.
    scale: i32,
    /// Column `i`'s runs over the ground are `over[over_at[i]..over_at[i + 1]]`,
    /// lowest first, none overlapping; the apron's have none.
    pub over_at: Vec<u32>,
    pub over: Vec<Over>,
    /// Each column's span under the ground, `[lo, hi)`, empty where none does.
    pub under: Vec<Under>,
    /// Each column's look: where a pier stands is the data's, and a void one has no ground.
    look: Vec<GroundCell>,
    crossing: Vec<(usize, usize, Crossing)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Under {
    pub span: [i32; 2],
    pub material: u32,
    pub layer: u8,
}

/// `lo` up to `hi` exclusive, in cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
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
            span: COL_SPAN,
            scale: 1,
            over_at: vec![0; COL_SPAN2 + 1],
            over: Vec::new(),
            under: vec![Under::default(); COL_SPAN2],
            look: vec![ground::BASE; COL_SPAN2],
            crossing: Vec::new(),
        }
    }
}

impl ChunkSample {
    /// Each cell as its source draws it (`TileSource::cell_at`). `span`
    /// columns a row, the apron's among them.
    pub fn read(&mut self, around: &Around, first: [i64; 2], span: u32) -> Result<(), Refused> {
        let tables = &*MATERIALS;
        (self.span, self.scale) = (span, 1);
        for py in 0..span {
            for px in 0..span {
                let i = (py * span + px) as usize;
                let (x, y) = (first[0] + px as i64 - APRON as i64, first[1] + py as i64 - APRON as i64);
                let [qx, qy] = ancestor_of([x, y], GROUND.tile_cell_shamt());
                let (source, [x, y]) = match (qx, qy) {
                    (0, 0) => (&around.own, [x, y]),
                    _ => match &around.beside[beside_index(qx, qy)] {
                        Some(s) => (s, offset_in([x, y], GROUND.tile_cell_shamt())),
                        None => (&around.own, [x, y]),
                    },
                };
                let (cell, ele) = source.cell_at([x, y]);
                self.look[i] = cell;
                (self.material[i], self.layer_idx[i]) = tables.drawn(cell);
                self.ele[i] = ele;
            }
        }
        self.read_separation(around, first)?;
        self.floor = self.floor_of();
        Ok(())
    }

    /// The chunk's own: a neighbor's valley must not drop it. A chunk with no
    /// surface has no floor: nothing of it is drawn from one.
    fn floor_of(&self) -> i32 {
        let own = APRON..self.span - APRON;
        let mut lowest = i32::MAX;
        for py in own.clone() {
            for px in own.clone() {
                let i = (py * self.span + px) as usize;
                if self.has_ground(i) {
                    lowest = lowest.min(self.ele[i]);
                }
                let b = self.under[i];
                if b.span[0] < b.span[1] {
                    lowest = lowest.min(b.span[0] - self.scale);
                }
            }
        }
        match lowest {
            i32::MAX => i32::MIN,
            lowest => lowest - TERRAIN_Z_MARGIN * self.scale,
        }
    }

    /// Each column's lines from the tile its height is read from (`read`).
    fn read_separation(&mut self, around: &Around, first: [i64; 2]) -> Result<(), Refused> {
        let mut crossing = core::mem::take(&mut self.crossing);
        crossing.clear();
        let per_m = u_per_m(around.own.frame.corner[1]);
        let corner = first.map(|v| v - APRON as i64);
        // A column with no tile beside is read from the chunk's own, as its height is.
        let beside = |[qx, qy]: [i64; 2]| if (qx, qy) == (0, 0) { None } else { around.beside[beside_index(qx, qy)].as_ref() };
        let span = self.span;
        let mut built = collect_separation(&mut crossing, &around.own, corner, [0, 0], span, |q| beside(q).is_none());
        for qy in -1..=1 {
            for qx in -1..=1 {
                if let (Some(s), true) = (beside([qx, qy]), built.is_ok()) {
                    built = collect_separation(&mut crossing, s, corner, [qx, qy], span, |q| q == [qx, qy]);
                }
            }
        }
        let built = built.and_then(|()| self.over_of(&mut crossing, per_m));
        self.crossing = crossing;
        built
    }

    fn over_of(&mut self, crossing: &mut Vec<(usize, usize, Crossing)>, per_m: f64) -> Result<(), Refused> {
        let tables = &*MATERIALS;
        // A line over a column is all its segments sweep there; a station is one segment's,
        // so the pier is either's.
        crossing.sort_by_key(|a| (a.0, a.1));
        crossing.dedup_by(|b, a| {
            let same = (a.0, a.1) == (b.0, b.1);
            if same {
                (a.2.low, a.2.top, a.2.pier) = (a.2.low.min(b.2.low), a.2.top.max(b.2.top), a.2.pier || b.2.pier);
            }
            same
        });
        self.over.clear();
        let (mut col, columns) = (0, (self.span * self.span) as usize);
        let mut i = 0;
        while col < columns {
            self.over_at[col] = self.over.len() as u32;
            self.under[col] = Under::default();
            let from = self.over.len();
            let col_from = i;
            while i < crossing.len() && crossing[i].0 == col {
                let o = crossing[i].2;
                i += 1;
                let Some((slab, slab_layer)) = tables.of(o.kind) else { continue };
                let ground = self.ele[col];
                let (slab_at, pier_at) = match o.occupies(ground, per_m) {
                    Occupies::Grade => {
                        (self.material[col], self.layer_idx[col]) = (slab, slab_layer);
                        continue;
                    }
                    Occupies::Under { span } => {
                        if span[1] - span[0] > self.under[col].span[1] - self.under[col].span[0] {
                            self.under[col] = Under {
                                span,
                                material: slab,
                                layer: slab_layer,
                            };
                        }
                        continue;
                    }
                    Occupies::Over { slab, pier } => (slab, pier),
                };
                let [lo, hi] = slab_at;
                try_push(
                    &mut self.over,
                    Over {
                        lo,
                        hi,
                        material: slab,
                        layer: slab_layer,
                    },
                )?;
                let column = crossing[col_from..].iter().take_while(|c| c.0 == col).map(|c| c.2);
                if let (Some([lo, hi]), Some((pier, pier_layer))) = (pier_at.and_then(|p| pier_stands(p, self.look[col], column, ground, per_m)), tables.of(PIER)) {
                    try_push(
                        &mut self.over,
                        Over {
                            lo,
                            hi,
                            material: pier,
                            layer: pier_layer,
                        },
                    )?;
                }
            }
            // Lowest first; a higher span keeps what two share.
            let column = &mut self.over[from..];
            column.sort_by_key(|s| (s.lo, s.hi));
            for k in (1..column.len()).rev() {
                column[k - 1].hi = column[k - 1].hi.min(column[k].lo);
            }
            let mut k = from;
            for j in from..self.over.len() {
                if self.over[j].hi > self.over[j].lo {
                    self.over[k] = self.over[j];
                    k += 1;
                }
            }
            self.over.truncate(k);
            col += 1;
        }
        self.over_at[columns] = self.over.len() as u32;
        Ok(())
    }

    /// The sample of a chunk `k` zooms finer than the one `leaf` was read for,
    /// `span` columns a row, its first column `at` its own cells into that
    /// chunk: each column is the leaf column it lies in, heights in its own
    /// cells. Terrain is drawn as finely as it is held, so nothing is invented.
    pub fn refine(&mut self, leaf: &ChunkSample, k: u32, at: [u32; 2], span: u32) -> Result<(), Refused> {
        let scale = 1i32 << k;
        (self.span, self.scale) = (span, leaf.scale * scale);
        let in_leaf = |v: u32, p: u32| (i64::from(v) + i64::from(p) - i64::from(APRON)).div_euclid(i64::from(scale)) + i64::from(APRON);
        self.over.clear();
        for py in 0..span {
            for px in 0..span {
                let i = (py * span + px) as usize;
                let j = (in_leaf(at[1], py) * i64::from(leaf.span) + in_leaf(at[0], px)) as usize;
                self.material[i] = leaf.material[j];
                self.layer_idx[i] = leaf.layer_idx[j];
                self.look[i] = leaf.look[j];
                self.ele[i] = leaf.ele[j] * scale;
                let b = leaf.under[j];
                self.under[i] = Under { span: b.span.map(|v| v * scale), ..b };
                self.over_at[i] = self.over.len() as u32;
                for s in &leaf.over[leaf.over_at[j] as usize..leaf.over_at[j + 1] as usize] {
                    try_push(
                        &mut self.over,
                        Over {
                            lo: s.lo * scale,
                            hi: s.hi * scale,
                            ..*s
                        },
                    )?;
                }
            }
        }
        self.over_at[(span * span) as usize] = self.over.len() as u32;
        self.floor = self.floor_of();
        Ok(())
    }

    /// The column's ground as inclusive runs: a top face at `ele` is cells up to
    /// `ele − 1` (`hi − 1` below). Through a bore: the ground under its floor,
    /// the way on the floor's cell, and the ground standing over its air.
    /// Whether column `i` has ground: a cell that is no look, void or the
    /// unknown past the coverage, gives no surface.
    pub fn has_ground(&self, i: usize) -> bool {
        Ground::of(self.look[i]).is_some()
    }

    pub fn ground_runs<E>(&self, i: usize, from: i32, to: i32, mut run: impl FnMut(i32, i32, u32, u8) -> Result<(), E>) -> Result<(), E> {
        if !self.has_ground(i) {
            return Ok(());
        }
        let top = self.ele[i].min(to);
        let (m, l) = (self.material[i], self.layer_idx[i]);
        let mut part = |lo: i32, hi: i32, m: u32, l: u8| if lo.max(from) < hi { run(lo.max(from), hi - 1, m, l) } else { Ok(()) };
        let b = self.under[i];
        if b.span[0] >= b.span[1] {
            return part(from, top, m, l);
        }
        let floor = b.span[0];
        part(from, (floor - self.scale).min(top), m, l)?;
        part(floor - self.scale, floor.min(top), b.material, b.layer)?;
        part(b.span[1], top, m, l)
    }

    pub fn column_runs<E>(&self, i: usize, mut run: impl FnMut(i32, i32, u32, u8) -> Result<(), E>) -> Result<(), E> {
        self.ground_runs(i, self.floor, i32::MAX, &mut run)?;
        for s in &self.over[self.over_at[i] as usize..self.over_at[i + 1] as usize] {
            run(s.lo, s.hi - 1, s.material, s.layer)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "sample.test.rs"]
mod test;
