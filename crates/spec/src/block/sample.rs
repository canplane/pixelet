// pxl-spec/block/sample.rs

//! What is in each block of the world, by one rule
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): the ground's
//! grid, each cell a block shifted up to its height (`bicubic`); then the caves, the ways off the ground and what is
//! placed, each written over the one before. `at` answers one block.

use pxl_portable::ways::{self, Columns, Segment, Stations};

use crate::bake::tile::terrain::GroundId;
use crate::block::cube::sentinel;

use crate::block::Block;
use crate::procedural::Frame;
use crate::space::coord::zoom::{units, Zoom, SHARD_ZOOM};
use crate::space::hierarchy::tree::ancestor_of;
use crate::space::layer::lattice::{GROUND, TILE_CELLS};
use pxl_portable::area::edge;
use pxl_portable::ground::{blocks, coarser, face_height, read_cell, Cell};
use pxl_portable::wrap_index;

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

/// The tile drawn in place of this one (its own, or a coarser one whose cell
/// stands for `n` × `n` of its cells until its own comes).
#[derive(Clone, Copy)]
pub struct TileSource<'a> {
    /// Its lines, which a client reads of a tile whole; its cells are its
    /// windows' (`Cells`).
    pub separation: &'a crate::bake::separation::Separation,
    /// Where the tile lies, for its piers' stations.
    pub frame: Frame,
    pub origin: [i64; 2],
    pub n: u32,
}

/// The ground's cells as a reader holds them (pxl-kernel
/// layer/terrain/window.rs).
pub trait Cells {
    /// The cell `g` at zoom `cell`, off the finest window holding it present
    /// (pxl-portable `window::find`): its four looks, its height and the zoom
    /// of the cell the word is, a coarser window's where one stands in;
    /// `void` at sea level where none does.
    fn cell(&self, cell: Zoom, g: [i32; 2]) -> (u32, i32, Zoom);
}

pub struct Around<'a, C> {
    pub cells: C,
    /// The asked tile's first cell, at its own cells' zoom `zoom`.
    pub first: [i64; 2],
    pub zoom: Zoom,
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

/// The asked tile's cells `x0..` of row `y`, counted from its first, into
/// `out`: each its looks, its height in leaf cells and its word's zoom, off
/// the windows, beside it as in it; `void` where nothing is known.
fn read_row(around: &Around<impl Cells>, y: i64, x0: i64, out: &mut [(u32, i32, Zoom)]) {
    for (v, x) in out.iter_mut().zip(x0..) {
        let g = [wrap_index(around.zoom, (around.first[0] + x) as i32), (around.first[1] + y) as i32];
        *v = around.cells.cell(around.zoom, g);
    }
}

/// Where a tile the ways are read from lies under a grid: the grid's first
/// column's corner in its cells, a column's side there, and its piers'
/// stations.
#[derive(Clone, Copy, Debug, Default)]
struct Place {
    origin: [f64; 2],
    step: f64,
    stations: Stations,
}

/// A window of the ground's grid at one zoom, each cell a block shifted up
/// to its height and what its top block shows, and the ways off the ground
/// read from the same tiles, each block asked of them where it is. Coarser than the
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
    /// The ways off the ground, as the one rule asks them of a block
    /// (pxl-portable ways.rs `column`, `block`), over the way columns: the
    /// grid's cells, or past the leaves the drawn tile's, which the cells
    /// finer than them read as the ground's do — `wshift` zooms coarser,
    /// `wspan` a side from `wfirst`. Each way column's segments with the
    /// point each starts at, column by column and a column's in its lines'
    /// points' order, a column's first at `starts`; the tile a column's are
    /// read from, the own `0` and the one beside at `q` `1 + beside_index(q)`,
    /// and where each lies; units of height a ground metre on the own tile.
    wshift: u32,
    wfirst: [i64; 2],
    wspan: u32,
    over: Vec<(u32, u32, Segment)>,
    starts: Vec<u32>,
    source: Vec<u8>,
    places: [Place; 9],
    per_m: f32,
    /// What a read works in, kept between reads: each cell's look as read,
    /// and the drawn tile's cells the window folds.
    look: Vec<GroundId>,
    cells: Vec<(u32, i32, Zoom)>,
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
            over: Vec::new(),
            starts: Vec::new(),
            source: Vec::new(),
            places: [Place::default(); 9],
            per_m: 0.0,
            wshift: 0,
            wfirst: [0; 2],
            wspan: 0,
            look: Vec::new(),
            cells: Vec::new(),
        }
    }
}

impl Grid {
    /// The `span` × `span` cells at `cell` from `first`, read off `around`,
    /// whose own tile is at `tile` with its first cell, at `cell`, at
    /// `tile_first`. Past the service area there is no ground, and at the
    /// leaves the area is walled from inside (`wall_off`).
    pub fn read(&mut self, around: &Around<impl Cells>, cell: Zoom, tile: Zoom, tile_first: [i64; 2], first: [i64; 2], span: u32) -> Result<(), Refused> {
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
        fit(&mut self.cells, across * across, (u32::from(sentinel::void), 0, cell))?;
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
            let c = [0, 1].map(|a| drawn(rel[a] + g[a]));
            let (look, h, fold) = if k == 1 {
                coarser([[0, 0], [1, 0], [0, 1], [1, 1]].map(|[dx, dy]| read([c[0] + dx, c[1] + dy])), cell)
            } else {
                let (w, h, from) = read(c);
                read_cell(w, h, from)
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
            let b = [(self.first[0] + x) as i32, (self.first[1] + y) as i32];
            *look = byte(edge(u32::from(g), self.cell, b, g == sentinel::unknown, |dx, dy| past(x + i64::from(dx), y + i64::from(dy))));
        }
    }

    /// Each cell's segments off the lines of the tile its height is read
    /// from, for the one rule to ask: a way at grade recolors its face, a
    /// bore takes the ground out over its floor, and a deck and its pier
    /// stand over the ground.
    fn ways(&mut self, around: &Around<impl Cells>, tile_first: [i64; 2], k: i32) -> Result<(), Refused> {
        let mut over = core::mem::take(&mut self.over);
        over.clear();
        // Past the leaves a way column is a drawn tile's cell, as the ground's is.
        let wshift = (-k).max(0) as u32;
        let wfirst = self.first.map(|v| v >> wshift);
        let wspan = (0..2).map(|a| ((self.first[a] + i64::from(self.span) - 1) >> wshift) - wfirst[a] + 1).max().unwrap_or(0) as u32;
        (self.wshift, self.wfirst, self.wspan) = (wshift, wfirst, wspan);
        let (span, n) = (i64::from(wspan), (wspan * wspan) as usize);
        fit(&mut self.source, n, 0)?;
        let source = &mut self.source;
        let places = &mut self.places;
        let rel = [wfirst[0] - (tile_first[0] >> wshift), wfirst[1] - (tile_first[1] >> wshift)];
        // A way column's side and its first's corner, in the drawn tile's cells.
        let step = if k >= 0 { f64::from(1 << k) } else { 1.0 };
        let corner = rel.map(|v| v as f64 * step);
        let quadrant = |c: [i64; 2]| ancestor_of([0, 1].map(|a| ((rel[a] + c[a]) as f64 * step).floor() as i64), GROUND.tile_cell_shamt());
        let beside = |q: [i64; 2]| if q == [0, 0] { None } else { around.beside[beside_index(q[0], q[1])].as_ref() };
        let mut read = |s: &TileSource, q: [i64; 2], at: usize, reads: &dyn Fn([i64; 2]) -> bool| -> Result<(), Refused> {
            let n = f64::from(s.n);
            let origin = [0, 1].map(|a| (s.origin[a] as f64 + corner[a] - (q[a] * i64::from(TILE_CELLS)) as f64) / n);
            places[at] = Place {
                origin,
                step: step / n,
                stations: s.frame.stations(),
            };
            let mut grown = Ok(());
            s.separation.segments_over(origin, step / n, [span, span], |cx, cy, p| {
                if grown.is_ok() && reads(quadrant([cx, cy])) {
                    let col = (cy * span + cx) as usize;
                    source[col] = at as u8;
                    grown = try_push(&mut over, (col as u32, p as u32, s.separation.segment(p)));
                }
            });
            grown
        };
        // A cell with no tile beside is read from the own tile, as its height is.
        read(&around.own, [0, 0], 0, &|q| beside(q).is_none())?;
        for qy in -1..=1 {
            for qx in -1..=1 {
                if let Some(s) = beside([qx, qy]) {
                    read(s, [qx, qy], 1 + beside_index(qx, qy), &|q| q == [qx, qy])?;
                }
            }
        }
        // A cell's segments in the order its lines' points run.
        over.sort_by_key(|o| (o.0, o.1));
        fit(&mut self.starts, n + 1, 0)?;
        let mut at = 0;
        for (col, start) in self.starts[..=n].iter_mut().enumerate() {
            while at < over.len() && (over[at].0 as usize) < col {
                at += 1;
            }
            *start = at as u32;
        }
        self.over = over;
        self.per_m = around.own.frame.per_m();
        Ok(())
    }

    /// The cell `[x, y]` at its zoom, as an index, if the window holds it.
    pub fn index(&self, at: [i64; 2]) -> Option<usize> {
        let [x, y] = [at[0] - self.first[0], at[1] - self.first[1]];
        let span = i64::from(self.span);
        ((0..span).contains(&x) && (0..span).contains(&y)).then(|| self.local([x as i32, y as i32])).flatten()
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

    /// What the ways write in block `b` (pxl-portable ways.rs `block`): a
    /// deck's, a pier's or a bore's block, or a bore's air; none where they
    /// write nothing or the window does not hold it.
    fn way(&self, b: [i64; 3], answered_air: bool) -> Option<Option<GroundId>> {
        let i = self.index([b[0], b[1]])?;
        let z = b[2] as i32;
        let w = [((b[0] >> self.wshift) - self.wfirst[0]) as i32, ((b[1] >> self.wshift) - self.wfirst[1]) as i32];
        // Carved: air as the caves answer it where the grid's ground fills.
        let carved = answered_air && self.block(i, z).is_some();
        match ways::block(self, [w[0], w[1], z], carved) {
            ways::NOTHING => None,
            ways::AIR => Some(None),
            g => Some(Some(g as GroundId)),
        }
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
        if let Some(way) = self.way(b, g == u32::from(sentinel::void)) {
            return way.map(|g| Block::Ground(g, [g; 4]));
        }
        let g = byte(g);
        (g != sentinel::void).then_some(Block::Ground(g, fold.map(byte)))
    }
}

/// The window's columns as the ways' one rule reads them, counted from its
/// first; a column it does not hold has no segment.
impl Columns for Grid {
    fn cell(&self) -> Zoom {
        self.cell
    }

    fn per_m(&self) -> f32 {
        self.per_m
    }

    fn count(&self, c: [i32; 2]) -> u32 {
        self.wlocal(c).map_or(0, |i| self.starts[i + 1] - self.starts[i])
    }

    fn segment(&self, c: [i32; 2], k: u32) -> Segment {
        self.over[(self.starts[self.wlocal(c).unwrap_or(0)] + k) as usize].2
    }

    fn center(&self, c: [i32; 2]) -> [f32; 2] {
        let p = self.place(c);
        [0, 1].map(|a| (p.origin[a] + (f64::from(c[a]) + 0.5) * p.step) as f32)
    }

    fn side(&self, c: [i32; 2]) -> f32 {
        self.place(c).step as f32
    }

    fn stations(&self, c: [i32; 2]) -> Stations {
        self.place(c).stations
    }

    fn look(&self, c: [i32; 2]) -> u32 {
        self.cell_of(c).map_or(u32::from(sentinel::void), |i| u32::from(self.look[i]))
    }

    fn face(&self, c: [i32; 2]) -> i32 {
        self.cell_of(c).map_or(0, |i| self.face_height(i))
    }

    /// The segment starts at the point after the one before it, and
    /// continues its line's.
    fn joins(&self, c: [i32; 2], k: u32) -> bool {
        let at = (self.starts[self.wlocal(c).unwrap_or(0)] + k) as usize;
        k > 0 && self.over[at].1 == self.over[at - 1].1 + 1 && self.over[at].2.continues == 1
    }
}

/// What the ways leave in the window's columns, each read off its spans.
impl ways::Airs for Grid {
    fn at(&self, c: [i32; 2], z: i32) -> u32 {
        ways::at(&ways::column(self, c).0, self.cell, z)
    }
}

impl Grid {
    /// The column `c` counted from the first, as an index, if held.
    fn local(&self, c: [i32; 2]) -> Option<usize> {
        let span = self.span as i32;
        ((0..span).contains(&c[0]) && (0..span).contains(&c[1])).then(|| (c[1] * span + c[0]) as usize)
    }

    /// The way column `c` counted from `wfirst`, as an index, if held.
    fn wlocal(&self, c: [i32; 2]) -> Option<usize> {
        let span = self.wspan as i32;
        ((0..span).contains(&c[0]) && (0..span).contains(&c[1])).then(|| (c[1] * span + c[0]) as usize)
    }

    /// A cell of the grid way column `c` holds, which all it holds read as
    /// one: its first the grid holds.
    fn cell_of(&self, c: [i32; 2]) -> Option<usize> {
        let first = [0, 1].map(|a| (((self.wfirst[a] + i64::from(c[a])) << self.wshift) - self.first[a]).max(0) as i32);
        self.wlocal(c)?;
        self.local(first)
    }

    /// Where way column `c`'s segments' tile lies.
    fn place(&self, c: [i32; 2]) -> &Place {
        &self.places[usize::from(self.wlocal(c).map_or(0, |i| self.source[i]))]
    }
}
