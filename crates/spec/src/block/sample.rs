// pxl-spec/block/sample.rs

//! A terrain tile's chunk as its blocks are composed from it: per cell one
//! look and one top, a column from a floor shared by the chunk. Bridges and
//! tunnels stand off the column as runs of their own; a bore or a cutting
//! takes the air over its floor out of the column.

use crate::bake::separation::{Crossing, Occupies, Separation};
use crate::bake::surface::bicubic;
use crate::bake::tile::terrain::GroundId;
use crate::block::cube::{ground, sentinel, Ground};
use crate::block::limit::{bedrock, wall};
use crate::procedural::{pier_stands, Caves, Frame};
use crate::space::camera::grid::CHUNK_STEPS;
use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::{count, Zoom};
use crate::space::hierarchy::tree::{ancestor_of, offset_in};
use crate::space::layer::lattice::{GROUND, GROUND_FOLD, TILE_CELLS};

/// Minecraft Beta's filler under a column's face: `stoneNoise / 3 + 3`, its
/// noise left out for now.
const FILLER_DEPTH: i32 = 3;

/// Its sandstone under sand: `rand.nextInt(4)`, here its mean rounded up.
const SANDSTONE_DEPTH: i32 = 2;

/// The ring of neighbor columns a chunk is composed with. Without it the face
/// mask read every chunk seam as open air, losing occlusion, creases and
/// culling there. Nothing is emitted from an apron column.
pub const APRON: u32 = 1;

/// Columns a row of a node at `zoom` composed at `runs`, the apron's among them.
pub const fn span(zoom: Zoom, runs: Zoom) -> u32 {
    count(runs, zoom) + 2 * APRON
}

/// Column `(cx, cy)` of the chunk's own in a sample `span` wide.
pub fn apron_col(span: u32, cx: u32, cy: u32) -> u32 {
    (cy + APRON) * span + cx + APRON
}

/// The widest: a chunk's columns where they fold into its cells, and its apron.
pub const COL_SPAN: u32 = span(0, CHUNK_STEPS + GROUND_FOLD);

// A composition's workspace grows with what stands in the chunk, so it is the
// heap's. Growth asks for room first; a refused one ends, what is drawn stays
// drawn, and it is asked for again later, rather than the allocator ending the module.

/// The heap refused a composition's workspace room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refused;

impl From<std::collections::TryReserveError> for Refused {
    fn from(_: std::collections::TryReserveError) -> Refused {
        Refused
    }
}

pub fn grow<T>(v: &mut Vec<T>, more: usize) -> Result<(), Refused> {
    let need = v.len() + more;
    if need > v.capacity() {
        v.try_reserve(need.max(2 * v.len()) - v.len())?;
    }
    Ok(())
}

pub fn try_push<T>(v: &mut Vec<T>, x: T) -> Result<(), Refused> {
    grow(v, 1)?;
    v.push(x);
    Ok(())
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
const PIER: GroundId = ground::building;

#[derive(Clone, Copy)]
pub struct TileView<'a> {
    pub cells: &'a [GroundId],
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
    pub fn cell_at(&self, at: [i64; 2]) -> (GroundId, i32) {
        let last = TILE_CELLS as i32 - 1;
        let (tile, n) = (&self.tile, self.n);
        let (x, y) = (self.origin[0] + at[0], self.origin[1] + at[1]);
        let (tx, ty) = (x.div_euclid(n as i64) as i32, y.div_euclid(n as i64) as i32);
        // Only the elevation is padded past the tile. An apron cell off it keeps its
        // height, all the face mask reads, and the base look nothing draws.
        let on_tile = (0..=last).contains(&tx) && (0..=last).contains(&ty);
        let cell = if on_tile { tile.cells[GROUND.cell_index(tx as u32, ty as u32) as usize] } else { ground::BASE };
        let center = |v: i64| (v as f64 + 0.5) / n as f64;
        // A height h is cells up to h − 1 of ground, its surface at z = h.
        (cell, bicubic(tile.ele, center(x), center(y)) as i32)
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
    /// What each column's ground is drawn as: its look, or a way's at grade.
    pub block: Vec<GroundId>,
    /// In cells; a column's top is one below it.
    pub ele: Vec<i32>,
    /// Columns a row, its own and the apron's (`apron_col`).
    pub span: u32,
    /// Its cells a leaf cell's side: past the leaves, a way's slab stays a leaf cell thick.
    scale: i32,
    /// Its first column's corner and a column's side, in units: where a cave
    /// is is the world's (`hollow`), not a chunk's.
    corner: [f64; 2],
    side: f64,
    /// The zoom of the cells drawn from it, which decides whether its ground
    /// is laid in strata (`strata`): a fold one finer reads the leaves, and
    /// is a summary all the same.
    cell: Zoom,
    /// Column `i`'s runs over the ground are `over[over_at[i]..over_at[i + 1]]`,
    /// lowest first, none overlapping; the apron's have none.
    pub over_at: Vec<u32>,
    pub over: Vec<Over>,
    /// Each column's span under the ground, `[lo, hi)`, empty where none does.
    pub under: Vec<Under>,
    /// Each column's look: where a pier stands is the data's, and a void one has no ground.
    look: Vec<GroundId>,
    crossing: Vec<(usize, usize, Crossing)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Under {
    pub span: [i32; 2],
    pub block: GroundId,
}

/// `lo` up to `hi` exclusive, in cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    pub lo: i32,
    pub hi: i32,
    pub block: GroundId,
}

impl ChunkSample {
    /// Room for `span` columns a row at most.
    pub fn of_span(span: u32) -> ChunkSample {
        let n = (span * span) as usize;
        ChunkSample {
            block: vec![ground::BASE; n],
            ele: vec![0; n],
            span,
            scale: 1,
            corner: [0.0; 2],
            side: 1.0,
            cell: GROUND.cell(),
            over_at: vec![0; n + 1],
            over: Vec::new(),
            under: vec![Under::default(); n],
            look: vec![ground::BASE; n],
            crossing: Vec::new(),
        }
    }

    /// Each cell as its source draws it (`TileSource::cell_at`), for cells
    /// drawn at `cell`. `span` columns a row, the apron's among them.
    pub fn read(&mut self, around: &Around, first: [i64; 2], span: u32, cell: Zoom) -> Result<(), Refused> {
        (self.span, self.scale, self.cell) = (span, 1, cell);
        let own = &around.own;
        self.side = own.frame.cell / f64::from(own.n);
        self.corner = [0, 1].map(|k| own.frame.corner[k] + (own.origin[k] + first[k] - i64::from(APRON)) as f64 * self.side);
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
                (self.look[i], self.block[i]) = (cell, cell);
                self.ele[i] = ele;
            }
        }
        // Which cells a source read, not the apron's stand-in past a tile not held.
        let read = |i: usize| {
            let (px, py) = (i as i64 % i64::from(span), i as i64 / i64::from(span));
            let at = [first[0] + px - i64::from(APRON), first[1] + py - i64::from(APRON)];
            let [qx, qy] = ancestor_of(at, GROUND.tile_cell_shamt());
            (qx, qy) == (0, 0) || around.beside[beside_index(qx, qy)].is_some()
        };
        self.wall_off(read);
        self.read_separation(around, first)?;
        Ok(())
    }

    /// Each column with no ground walled where ground is beside it (`wall`):
    /// bedrock, `unknown`, up to that ground's face, and any other left void.
    /// A wall is no ground beside another, so one pass lays it one cell thick.
    /// Only what a source `read` is known: a stand-in neither walls nor is.
    fn wall_off(&mut self, read: impl Fn(usize) -> bool) {
        let span = self.span as i64;
        for i in 0..(span * span) as usize {
            if !read(i) || Ground::of(self.look[i]).is_some() {
                continue;
            }
            let (x, y) = (i as i64 % span, i as i64 / span);
            let beside = (-1..=1)
                .flat_map(|dy| (-1..=1).map(move |dx| (x + dx, y + dy)))
                .filter(|&(bx, by)| (0..span).contains(&bx) && (0..span).contains(&by));
            let faces = beside
                .map(|(bx, by)| (by * span + bx) as usize)
                .filter(|&j| read(j) && Ground::of(self.look[j]).is_some())
                .map(|j| self.ele[j]);
            let (look, ele) = wall(faces).map_or((sentinel::void, self.ele[i]), |face| (sentinel::unknown, face));
            (self.look[i], self.block[i], self.ele[i]) = (look, look, ele);
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
                if Ground::of(o.kind).is_none() {
                    continue;
                }
                let ground = self.ele[col];
                let (slab_at, pier_at) = match o.occupies(ground, per_m) {
                    Occupies::Grade => {
                        self.block[col] = o.kind;
                        continue;
                    }
                    Occupies::Under { span } => {
                        if span[1] - span[0] > self.under[col].span[1] - self.under[col].span[0] {
                            self.under[col] = Under { span, block: o.kind };
                        }
                        continue;
                    }
                    Occupies::Over { slab, pier } => (slab, pier),
                };
                let [lo, hi] = slab_at;
                try_push(&mut self.over, Over { lo, hi, block: o.kind })?;
                let column = crossing[col_from..].iter().take_while(|c| c.0 == col).map(|c| c.2);
                if let Some([lo, hi]) = pier_at.and_then(|p| pier_stands(p, self.look[col], column, ground, per_m)) {
                    try_push(&mut self.over, Over { lo, hi, block: PIER })?;
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
        (self.span, self.scale, self.cell) = (span, leaf.scale * scale, leaf.cell + k as Zoom);
        self.side = leaf.side / f64::from(scale);
        self.corner = [0, 1].map(|n| leaf.corner[n] + f64::from(APRON) * leaf.side + (f64::from(at[n]) - f64::from(APRON)) * self.side);
        let in_leaf = |v: u32, p: u32| (i64::from(v) + i64::from(p) - i64::from(APRON)).div_euclid(i64::from(scale)) + i64::from(APRON);
        self.over.clear();
        for py in 0..span {
            for px in 0..span {
                let i = (py * span + px) as usize;
                let j = (in_leaf(at[1], py) * i64::from(leaf.span) + in_leaf(at[0], px)) as usize;
                self.block[i] = leaf.block[j];
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
        Ok(())
    }

    /// Whether column `i` has ground: a look, or the bedrock walling the
    /// service area (`wall_off`); a void column gives no surface.
    pub fn has_ground(&self, i: usize) -> bool {
        Ground::of(self.look[i]).is_some() || self.look[i] == sentinel::unknown
    }

    /// The world's caves down column `i`, asked of a cell `z` by its center
    /// (`cell_z`).
    fn caves(&self, i: usize) -> Caves {
        let span = self.span as usize;
        let at = [i % span, i / span].map(|v| v as f64 + 0.5);
        Caves::column(self.corner[0] + at[0] * self.side, self.corner[1] + at[1] * self.side)
    }

    /// The center of the cell `z`, in units.
    fn cell_z(&self, z: i32) -> f64 {
        (f64::from(z) + 0.5) / f64::from(self.scale)
    }

    /// Whether a way runs in column `i` or one beside it — at grade, through a
    /// bore or over a slab — where a cave opens nowhere: a way and the ground
    /// about it win over it. Beside is the apron's reach.
    fn by_a_way(&self, i: usize) -> bool {
        let span = self.span as i64;
        let (x, y) = (i as i64 % span, i as i64 / span);
        let way = |j: usize| Ground::of(self.block[j]).is_some_and(Ground::transport) || self.under[j].span[0] < self.under[j].span[1] || self.over_at[j] < self.over_at[j + 1];
        (-1..=1).any(|dy| (-1..=1).any(|dx| (0..span).contains(&(x + dx)) && (0..span).contains(&(y + dy)) && way(((y + dy) * span + x + dx) as usize)))
    }

    /// The column's ground as inclusive runs: a top face at `ele` is cells up to
    /// `ele − 1` (`hi − 1` below). Through a bore: the ground under its floor,
    /// the way on the floor's cell, and the ground standing over its air.
    /// Caves are taken out of it where the world has them (`hollow`), in a
    /// leaf's cells or finer as strata are, last of all: bedrock, the ground's
    /// face and the ways win over them, so a cave never opens the face, takes
    /// bedrock or comes by a way.
    pub fn ground_runs<E>(&self, i: usize, from: i32, to: i32, mut run: impl FnMut(i32, i32, GroundId) -> Result<(), E>) -> Result<(), E> {
        if !self.has_ground(i) {
            return Ok(());
        }
        let top = self.ele[i].min(to);
        let mut caves = (self.cell >= GROUND.cell() && self.look[i] != sentinel::unknown && !self.by_a_way(i)).then(|| self.caves(i));
        let mut part = |lo: i32, hi: i32, g: GroundId| {
            let lo = lo.max(from);
            let Some(caves) = caves.as_mut().filter(|_| g != sentinel::unknown) else {
                return if lo < hi { run(lo, hi - 1, g) } else { Ok(()) };
            };
            // Each stretch of the cells the world leaves solid, a knot interval
            // at a time where it is sure of all its cells (`Caves::stretch`).
            let mut start = None;
            let face = self.ele[i] - self.scale;
            let mut set = |z: i32, hollow: bool, start: &mut Option<i32>| match (hollow, *start) {
                (true, Some(s)) => {
                    *start = None;
                    run(s, z - 1, g)
                }
                (false, None) => {
                    *start = Some(z);
                    Ok(())
                }
                _ => Ok(()),
            };
            let mut z = lo;
            while z < hi {
                if z >= face {
                    set(z, false, &mut start)?;
                    break;
                }
                let (end, sure) = caves.stretch(self.cell_z(z));
                // The first cell whose center is at or past the interval's end.
                let next = ((end * f64::from(self.scale) - 0.5).ceil() as i32).clamp(z + 1, hi.min(face));
                match sure {
                    Some(hollow) => set(z, hollow, &mut start)?,
                    None => {
                        for z in z..next {
                            set(z, caves.hollow(self.cell_z(z)), &mut start)?;
                        }
                    }
                }
                z = next;
            }
            match start {
                Some(s) => run(s, hi - 1, g),
                None => Ok(()),
            }
        };
        let strata = self.strata(i);
        // Each stratum's cells in `[lo, hi)`, its own bottom up to the one over
        // it, lowest first; strata of one look over each other are one, as sand
        // lies on sand.
        let layered = |lo: i32, hi: i32, part: &mut dyn FnMut(i32, i32, GroundId) -> Result<(), E>| {
            let mut k = strata.len();
            while k > 0 {
                k -= 1;
                let mut top = k;
                while top > 0 && strata[top - 1].1 == strata[k].1 {
                    top -= 1;
                }
                let over = if top == 0 { i32::MAX } else { strata[top - 1].0 };
                part(lo.max(strata[k].0), hi.min(over), strata[k].1)?;
                k = top;
            }
            Ok(())
        };
        let b = self.under[i];
        if b.span[0] >= b.span[1] {
            return layered(from, top, &mut part);
        }
        let floor = b.span[0];
        layered(from, (floor - self.scale).min(top), &mut part)?;
        part(floor - self.scale, floor.min(top), b.block)?;
        layered(b.span[1], top, &mut part)
    }

    /// What column `i`'s ground is from its face down, each stratum's bottom
    /// and look, as Minecraft Beta lays a column
    /// (`ChunkProviderGenerate.replaceBlocksForBiome`): its look on top, its
    /// filler under it, sand over sandstone, and stone under all, every depth
    /// a leaf cell thick in any zoom's cells, and bedrock under all
    /// (`bedrock`). Strata, bedrock too, are a leaf cell's: a cell coarser
    /// than the leaves' is a summary of the faces under it, its ground its
    /// look whole, where a cell taller than the bedrock's depth folded it
    /// black under every far face.
    fn strata(&self, i: usize) -> [(i32, GroundId); 5] {
        let (look, s) = (self.block[i], self.scale);
        if look == sentinel::unknown || self.cell < GROUND.cell() {
            return [(i32::MIN, look); 5];
        }
        let rock = bedrock(self.ele[i], s);
        let face = self.ele[i] - s;
        let (filler, sandstone) = match look {
            ground::sand | ground::beach => (ground::sand, SANDSTONE_DEPTH),
            _ => (ground::soil, 0),
        };
        let filled = face - FILLER_DEPTH * s;
        [
            (face, look),
            (filled, filler),
            (filled - sandstone * s, ground::sandstone),
            (rock, ground::stone),
            (i32::MIN, sentinel::unknown),
        ]
    }
}

#[cfg(test)]
#[path = "sample.test.rs"]
mod test;
