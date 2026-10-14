// pxl-spec/block/sample.rs

//! What is in each block of the world, by one rule
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): the ground's
//! grid, each cell a block shifted up to its height (`bicubic`); then the caves, the ways off the ground and what is
//! placed, each written over the one before. `at` answers one block, and
//! `Blocks` holds a chunk's as a drawing of it asks them of the world.

use fnv::FnvHashMap;

use crate::bake::separation::{Crossing, Occupies};
use crate::bake::tile::terrain::GroundId;
use crate::block::cube::{ground, sentinel, Ground};
use crate::block::limit::at_edge;
use crate::block::Block;
use crate::procedural::{pier_stands, Frame};
use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::{units, Zoom, SHARD_ZOOM};
use crate::space::hierarchy::tree::{ancestor_of, offset_in};
use crate::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_portable::area::wall;
use pxl_portable::ground::{blocks, face_height, fold, read, Cell};

/// A building's concrete: nothing in the catalog is a pier's own.
const PIER: GroundId = ground::building;

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

/// `v` holding `n`, each new one `fill`, room asked for first.
fn fit<T: Clone>(v: &mut Vec<T>, n: usize, fill: T) -> Result<(), Refused> {
    if n > v.len() {
        grow(v, n - v.len())?;
    }
    v.resize(n, fill);
    Ok(())
}

#[derive(Clone, Copy)]
pub struct TileView<'a> {
    pub cells: &'a [GroundId],
    pub heights: &'a [i16],
    /// Its ways' bound by square and its bound's levels (pxl-spec
    /// bake/tile/terrain.rs `levels`), made from its lines and cells.
    pub ways: &'a [[i16; 2]],
    pub levels: &'a [i32],
    pub separation: &'a crate::bake::separation::Separation,
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

impl pxl_portable::ground::Tile for TileView<'_> {
    fn look(&self, x: u32, y: u32) -> u32 {
        u32::from(self.cells[GROUND.cell_index(x, y) as usize])
    }
    fn height(&self, x: u32, y: u32) -> i32 {
        i32::from(self.heights[GROUND.cell_index(x, y) as usize])
    }
}

/// The CPU draws the ways: a cell reaches its height and its square's ways.
impl pxl_portable::ground::Bound for TileView<'_> {
    fn cell(&self, x: u32, y: u32) -> [i32; 2] {
        crate::bake::tile::terrain::cell_bound(self.heights, self.ways, x, y)
    }
    fn level(&self, i: u32) -> i32 {
        self.levels[i as usize]
    }
}

pub struct Around<'a> {
    pub own: TileSource<'a>,
    /// None where the ring does not reach or nothing is drawn; those cells are
    /// `own`'s where it holds them, a coarser tile standing in, and else not
    /// known, no ground.
    pub beside: [Option<TileSource<'a>>; 8],
    /// Whether a point, in units, is past the service area: the one
    /// judgement (pxl-kernel io/source.rs `past_at`), asked of every cell
    /// whatever tile it is read from, or none, since a tile past the area is
    /// never held.
    pub past: fn([f64; 2]) -> bool,
}

pub const fn beside_index(qx: i64, qy: i64) -> usize {
    let k = (qx + 1) + 3 * (qy + 1);
    (if k > 4 { k - 1 } else { k }) as usize
}

impl<'a> Around<'a> {
    /// What draws the drawn tile's cell `at`, counted from the own tile's
    /// first: the own tile or the one beside it, the cell's place there, and
    /// which of the nine it is read as.
    fn of(&self, at: [i64; 2]) -> (&TileSource<'a>, [i64; 2], [i64; 2]) {
        let q = ancestor_of(at, GROUND.tile_cell_shamt());
        if q == [0, 0] {
            return (&self.own, at, q);
        }
        match &self.beside[beside_index(q[0], q[1])] {
            Some(s) => (s, offset_in(at, GROUND.tile_cell_shamt()), q),
            None => (&self.own, at, [0, 0]),
        }
    }
}

/// The drawn tile's cells `x0..` of row `y`, counted from the own tile's
/// first, into `out`: each its look and its height in leaf cells; `void`
/// where nothing is known.
fn read_row(around: &Around, y: i64, x0: i64, out: &mut [(u32, i32)]) {
    for (v, x) in out.iter_mut().zip(x0..) {
        let (s, at, _) = around.of([x, y]);
        *v = read(&s.tile, [0, 1].map(|a| (s.origin[a] + at[a]) as i32), s.n);
    }
}

/// A deck's slab or a pier in one cell: `lo` up to `hi` exclusive, in
/// blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slab {
    lo: i32,
    hi: i32,
    block: GroundId,
}

/// A window of the ground's grid at one zoom, each cell a block shifted up
/// to its height and what its top block shows, and the ways off the ground
/// read from the same tiles, written in 3D where they are. Coarser than the
/// leaves a cell folds the cells under it in 2D, its height their mean, its
/// look their majority and its color theirs (`fold`).
pub struct Grid {
    span: u32,
    cell: Zoom,
    /// Its first cell, at its zoom.
    first: [i64; 2],
    /// Blocks a leaf cell is tall, past the leaves; none where a cell folds
    /// the leaves, a summary of their faces laid in no strata.
    scale: Option<i32>,
    /// Each cell's ground under its face: the blocks under `top`.
    top: Vec<i32>,
    /// What its top block is: its look, a way at grade on it, or the wall.
    face: Vec<GroundId>,
    /// The looks it folds, four of the cells one zoom finer; its own look
    /// four times where it folds none.
    fold: Vec<[GroundId; 4]>,
    /// The ways off the ground by block: a deck's slab or a pier its block, a
    /// bore's air none, and the ground about a bore the way's, so a cave
    /// written under it never opens into it.
    ways: FnvHashMap<[i64; 3], Option<GroundId>>,
    /// What a read works in, kept between reads: each cell's look as read,
    /// the drawn tile's cells the window folds, the lines over each cell, a
    /// cell's decks and piers, and the bores' air with its way.
    look: Vec<GroundId>,
    cells: Vec<(u32, i32)>,
    crossing: Vec<(usize, usize, Crossing)>,
    slabs: Vec<Slab>,
    bores: Vec<([i64; 3], GroundId)>,
}

impl Default for Grid {
    fn default() -> Grid {
        Grid {
            span: 0,
            cell: GROUND.cell(),
            first: [0; 2],
            scale: Some(1),
            top: Vec::new(),
            face: Vec::new(),
            fold: Vec::new(),
            ways: FnvHashMap::default(),
            look: Vec::new(),
            cells: Vec::new(),
            crossing: Vec::new(),
            slabs: Vec::new(),
            bores: Vec::new(),
        }
    }
}

impl Grid {
    /// The `span` × `span` cells at `cell` from `first`, read off `around`,
    /// whose own tile is at `tile` with its first cell, at `cell`, at
    /// `tile_first`. Past the service area there is no ground, and at the
    /// leaves the area is walled from inside (`wall_off`).
    pub fn read(&mut self, around: &Around, cell: Zoom, tile: Zoom, tile_first: [i64; 2], first: [i64; 2], span: u32) -> Result<(), Refused> {
        let n = (span * span) as usize;
        fit(&mut self.top, n, 0)?;
        fit(&mut self.look, n, sentinel::void)?;
        fit(&mut self.face, n, sentinel::void)?;
        fit(&mut self.fold, n, [sentinel::void; 4])?;
        (self.span, self.cell, self.first) = (span, cell, first);
        let d = cell as i32 - GROUND.cell() as i32;
        self.scale = (d >= 0).then(|| 1 << d);
        // Drawn tile cells a cell folds, as a shift: past the leaves negative,
        // a drawn cell then spans several.
        let k = tile as i32 + GROUND.tile_cell_shamt() as i32 - cell as i32;
        xpute_kit::ensure!(k <= 1, EINVAL, k);
        let rel = [first[0] - tile_first[0], first[1] - tile_first[1]];
        let drawn = |g: i64| if k >= 0 { g << k } else { g >> -k };
        let from = rel.map(drawn);
        let across = match k {
            0.. => (span as usize) << k,
            _ => (0..2).map(|a| (drawn(rel[a] + i64::from(span) - 1) - from[a] + 1) as usize).max().unwrap_or(0),
        };
        fit(&mut self.cells, across * across, (u32::from(sentinel::void), 0))?;
        for (y, row) in self.cells.chunks_mut(across).take(across).enumerate() {
            read_row(around, from[1] + y as i64, from[0], row);
        }
        let side = units(cell);
        // The area runs on shards' edges, which no cell straddles: asked once a shard.
        let shard = units(SHARD_ZOOM);
        let mut asked: Option<([i64; 2], bool)> = None;
        for i in 0..n {
            let g = [(i % span as usize) as i64, (i / span as usize) as i64];
            let read = |c: [i64; 2]| self.cells[(c[1] - from[1]) as usize * across + (c[0] - from[0]) as usize];
            let (look, h, fold) = if k == 1 {
                let c = [0, 1].map(|a| drawn(rel[a] + g[a]));
                fold([[0, 0], [1, 0], [0, 1], [1, 1]].map(|[dx, dy]| read([c[0] + dx, c[1] + dy])), cell)
            } else {
                let (look, h) = read([0, 1].map(|a| drawn(rel[a] + g[a])));
                (look, h as f32, [look; 4])
            };
            let center = [0, 1].map(|a| (first[a] + g[a]) as f64 * side + side / 2.0);
            let key = center.map(|v| (v / shard).floor() as i64);
            let past = match asked {
                Some((at, past)) if at == key => past,
                _ => {
                    let past = (around.past)(center);
                    asked = Some((key, past));
                    past
                }
            };
            self.look[i] = if past { sentinel::unknown } else { byte(look) };
            self.fold[i] = fold.map(byte);
            self.top[i] = blocks(cell, h);
        }
        // As read, the area's edge marked: what `wall_off` reads.
        self.face[..n].copy_from_slice(&self.look[..n]);
        self.wall_off();
        self.face[..n].copy_from_slice(&self.look[..n]);
        self.ways(around, tile_first, k)
    }

    /// The service area walled from inside: at the leaves, each cell with
    /// ground beside one past the area is bedrock, `unknown`, up to its own
    /// face; past the area is void at every zoom.
    fn wall_off(&mut self) {
        let (span, n) = (i64::from(self.span), (self.span * self.span) as usize);
        // Read off `face`, still as read, so a wall laid is never read as
        // past the area.
        let marked = &self.face;
        let past = |x: i64, y: i64| (0..span).contains(&x) && (0..span).contains(&y) && marked[(y * span + x) as usize] == sentinel::unknown;
        for (i, (look, &g)) in self.look[..n].iter_mut().zip(&marked[..n]).enumerate() {
            let (x, y) = (i as i64 % span, i as i64 / span);
            *look = byte(wall(u32::from(g), self.cell, g == sentinel::unknown, at_edge(|dx, dy| past(x + dx, y + dy))));
        }
    }

    /// Each cell's ways off the lines of the tile its height is read from:
    /// a way at grade recolors its face, a bore takes the ground out over its
    /// floor, and a deck and its pier stand over the ground.
    fn ways(&mut self, around: &Around, tile_first: [i64; 2], k: i32) -> Result<(), Refused> {
        let mut crossing = core::mem::take(&mut self.crossing);
        crossing.clear();
        let span = i64::from(self.span);
        let rel = [self.first[0] - tile_first[0], self.first[1] - tile_first[1]];
        // A cell's side and its first's corner, in the drawn tile's cells.
        let step = if k >= 0 { f64::from(1 << k) } else { 1.0 / f64::from(1 << -k) };
        let corner = rel.map(|v| v as f64 * step);
        let quadrant = |c: [i64; 2]| ancestor_of([0, 1].map(|a| ((rel[a] + c[a]) as f64 * step).floor() as i64), GROUND.tile_cell_shamt());
        let beside = |q: [i64; 2]| if q == [0, 0] { None } else { around.beside[beside_index(q[0], q[1])].as_ref() };
        let mut read = |s: &TileSource, q: [i64; 2], reads: &dyn Fn([i64; 2]) -> bool| -> Result<(), Refused> {
            let n = f64::from(s.n);
            let origin = [0, 1].map(|a| (s.origin[a] as f64 + corner[a] - (q[a] * i64::from(TILE_CELLS)) as f64) / n);
            let mut grown = Ok(());
            s.tile.separation.crossing_grid(Some(s.frame), origin, step / n, [span, span], |cx, cy, d, o| {
                if grown.is_ok() && reads(quadrant([cx, cy])) {
                    grown = try_push(&mut crossing, ((cy * span + cx) as usize, d, o));
                }
            });
            grown
        };
        // A cell with no tile beside is read from the own tile, as its height is.
        read(&around.own, [0, 0], &|q| beside(q).is_none())?;
        for qy in -1..=1 {
            for qx in -1..=1 {
                if let Some(s) = beside([qx, qy]) {
                    read(s, [qx, qy], &|q| q == [qx, qy])?;
                }
            }
        }
        let built = self.ways_of(&mut crossing, u_per_m(around.own.frame.corner[1]));
        self.crossing = crossing;
        built
    }

    /// The ways over each cell, written by block: a deck's slab and its
    /// pier, lowest first so a higher one keeps what two share, over a
    /// bore's air; then about each bore's air the way's block where the
    /// ground is.
    fn ways_of(&mut self, crossing: &mut Vec<(usize, usize, Crossing)>, per_m: f64) -> Result<(), Refused> {
        // A line over a cell is all its segments sweep there; a station is
        // one segment's, so the pier is either's.
        crossing.sort_by_key(|a| (a.0, a.1));
        crossing.dedup_by(|b, a| {
            let same = (a.0, a.1) == (b.0, b.1);
            if same {
                (a.2.low, a.2.top, a.2.pier) = (a.2.low.min(b.2.low), a.2.top.max(b.2.top), a.2.pier || b.2.pier);
            }
            same
        });
        let (cell, span) = (self.cell, i64::from(self.span));
        let in_blocks = |v: [i32; 2]| v.map(|v| blocks(cell, v as f32));
        let mut ways = core::mem::take(&mut self.ways);
        let mut slabs = core::mem::take(&mut self.slabs);
        let mut bores = core::mem::take(&mut self.bores);
        ways.clear();
        bores.clear();
        let mut i = 0;
        let mut built = Ok(());
        while built.is_ok() && i < crossing.len() {
            let (col, first) = (crossing[i].0, i);
            let at = [self.first[0] + col as i64 % span, self.first[1] + col as i64 / span];
            slabs.clear();
            while i < crossing.len() && crossing[i].0 == col {
                let o = crossing[i].2;
                i += 1;
                // Bedrock wins over a way, as it does over the ground's face.
                if !Ground::grounds(u32::from(o.kind)) || self.look[col] == sentinel::unknown {
                    continue;
                }
                let ground = self.face_height(col);
                match o.occupies(ground, per_m) {
                    Occupies::Grade => self.face[col] = o.kind,
                    Occupies::Under { span } => {
                        let [lo, hi] = in_blocks(span);
                        for z in lo..hi {
                            built = built.and_then(|()| try_push(&mut bores, ([at[0], at[1], i64::from(z)], o.kind)));
                        }
                    }
                    Occupies::Over { slab, pier } => {
                        let [lo, hi] = in_blocks(slab);
                        built = built.and_then(|()| try_push(&mut slabs, Slab { lo, hi, block: o.kind }));
                        let lines = crossing[first..].iter().take_while(|c| c.0 == col).map(|c| c.2);
                        if let Some([lo, hi]) = pier.and_then(|p| pier_stands(p, self.look[col], lines, ground, per_m)).map(in_blocks) {
                            built = built.and_then(|()| try_push(&mut slabs, Slab { lo, hi, block: PIER }));
                        }
                    }
                }
            }
            for &(b, _) in bores.iter().rev().take_while(|(b, _)| [b[0], b[1]] == at) {
                built = built.and_then(|()| put(&mut ways, b, None));
            }
            slabs.sort_by_key(|o| (o.lo, o.hi));
            for o in &slabs {
                for z in o.lo..o.hi {
                    built = built.and_then(|()| put(&mut ways, [at[0], at[1], i64::from(z)], Some(o.block)));
                }
            }
        }
        // About a bore's air, where the ground is, the way's block.
        for &(b, way) in &bores {
            for d in NEAR {
                let at = [b[0] + d[0], b[1] + d[1], b[2] + d[2]];
                let ground = self.index([at[0], at[1]]).and_then(|j| self.block(j, at[2] as i32)).is_some();
                if ground && !ways.contains_key(&at) {
                    built = built.and_then(|()| put(&mut ways, at, Some(way)));
                }
            }
        }
        (self.ways, self.slabs, self.bores) = (ways, slabs, bores);
        built
    }

    /// The cell `[x, y]` at its zoom, as an index, if the window holds it.
    pub fn index(&self, at: [i64; 2]) -> Option<usize> {
        let span = i64::from(self.span);
        let [x, y] = [at[0] - self.first[0], at[1] - self.first[1]];
        ((0..span).contains(&x) && (0..span).contains(&y)).then(|| (y * span + x) as usize)
    }

    /// Whether cell `i` has ground: a look, or the bedrock walling the
    /// service area from inside; a void cell has none.
    fn grounded(&self, i: usize) -> bool {
        self.cell(i).grounded()
    }

    /// Its blocks under `top` are ground; none where it has none.
    pub fn top(&self, i: usize) -> Option<i32> {
        self.grounded(i).then_some(self.top[i])
    }

    /// What its top block is: its look, a way at grade on it, or the wall.
    pub fn face(&self, i: usize) -> GroundId {
        self.face[i]
    }

    /// What the ways write in block `b`: a deck's, a pier's or a bore's
    /// block, or a bore's air; none where they write nothing.
    fn way(&self, b: [i64; 3]) -> Option<Option<GroundId>> {
        self.ways.get(&b).copied()
    }

    /// Cell `i`'s face in leaf cells, as its top draws it: what the ways
    /// over it are told from, so they meet the ground drawn at every zoom
    /// (pxl-portable `face_height`).
    fn face_height(&self, i: usize) -> i32 {
        face_height(self.cell, self.top[i])
    }

    /// Cell `i`'s ground as the one rule takes it (pxl-portable `Cell`).
    fn cell(&self, i: usize) -> Cell {
        Cell {
            look: u32::from(self.face[i]),
            top: self.top[i],
            fold: self.fold[i].map(u32::from),
        }
    }

    /// What the ground is in block `z` of cell `i`: by depth under its top,
    /// none over it.
    fn block(&self, i: usize, z: i32) -> Option<GroundId> {
        Some(byte(self.cell(i).block(self.cell, z, false).0)).filter(|&g| g != sentinel::void)
    }
}

/// A look the one rule gives in a word, back to the byte it was read from.
fn byte(look: u32) -> GroundId {
    look as GroundId
}

/// `b` written as `block` into `ways`, room asked for first.
fn put(ways: &mut FnvHashMap<[i64; 3], Option<GroundId>>, b: [i64; 3], block: Option<GroundId>) -> Result<(), Refused> {
    if ways.len() == ways.capacity() {
        ways.try_reserve(ways.len().max(16))?;
    }
    ways.insert(b, block);
    Ok(())
}

impl Grid {
    /// The column `c` as the ground's one rule takes it (pxl-portable
    /// `Cell`, `ground::node`); none outside the grid.
    pub fn cell_at(&self, c: [i64; 2]) -> Option<Cell> {
        self.index(c).map(|i| self.cell(i))
    }

    /// Block `b` with what the ways write over the ground's block `ground`
    /// there (pxl-portable `Cell::block`), the layers read top down: what the
    /// ways write, air included; else the ground's. None is void or air.
    pub fn ways_over(&self, b: [i64; 3], (g, fold): (u32, [u32; 4])) -> Option<Block> {
        if let Some(way) = self.way(b) {
            return way.map(|g| Block::Ground(g, [g; 4]));
        }
        let g = byte(g);
        (g != sentinel::void).then_some(Block::Ground(g, fold.map(byte)))
    }
}

/// The six blocks across a block's faces.
const NEAR: [[i64; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Whether shading a block's faces reads the block at `o` about it
/// (`pxl-render mesh/chunk.rs` `mask`): every one beside it and over and
/// under those, the two over and under it, and the four at its corners and
/// over them; not the four under its corners, which no face's shading reads.
pub fn shades([x, y, z]: [i64; 3]) -> bool {
    let corner = x.abs() == 1 && y.abs() == 1;
    [x, y, z] != [0, 0, 0] && !(corner && z == -1)
}

/// A chunk's blocks as drawing what the rays stopped on in it reads them
/// (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md,
/// "Observation determines materialization"): the blocks the rays stopped
/// on, which alone are drawn, each whole, and the blocks about each its
/// shading reads (`shades`),
/// each as the world's one query answers it (`pxl-kernel query/block.rs`
/// `World::compose`); no other block is asked.
#[derive(Default)]
pub struct Blocks {
    /// The blocks the rays stopped on, each once.
    pub seen: Vec<[i64; 3]>,
    /// The chunk's first block, at its cells' zoom.
    first: [i64; 3],
    /// What each block asked is; none is void or air.
    asked: FnvHashMap<[i64; 3], Option<Block>>,
}

impl Blocks {
    /// The chunk whose first block is `first`, nothing asked yet.
    pub fn begin(&mut self, first: [i64; 3]) {
        self.seen.clear();
        self.asked.clear();
        self.first = first;
    }

    pub fn first(&self) -> [i64; 3] {
        self.first
    }

    /// Block `b` is one a ray stopped on.
    pub fn see(&mut self, b: [i64; 3]) -> Result<(), Refused> {
        try_push(&mut self.seen, b)
    }

    /// Whether block `b` was asked.
    pub fn asked(&self, b: [i64; 3]) -> bool {
        self.asked.contains_key(&b)
    }

    /// What block `b` is, as the world's one query answered it.
    pub fn put(&mut self, b: [i64; 3], block: Option<Block>) -> Result<(), Refused> {
        if self.asked.len() == self.asked.capacity() {
            self.asked.try_reserve(self.asked.len().max(16))?;
        }
        self.asked.insert(b, block);
        Ok(())
    }

    /// What block `b` is as it was answered, none for void or air; not
    /// asked, none of that either.
    pub fn block(&self, b: [i64; 3]) -> Option<Option<Block>> {
        self.asked.get(&b).copied()
    }
}
