// pxl-spec/block/sample.rs

//! The ground's grid as a reader reads a patch of it off the windows
//! (doc/minutes/20261012-the-ground-is-a-grid-then-overwrites.md): each
//! cell a block shifted up to its height, the service area walled from
//! inside (`Grid::read`). What overwrites it, the caves, the ways and what
//! is placed, a block reads by the world's one query (pxl-kernel
//! query/node.rs).

use crate::bake::tile::terrain::GroundId;
use crate::block::cube::sentinel;
use crate::space::coord::zoom::{units, Zoom, SHARD_ZOOM};
use crate::space::layer::lattice::GROUND;
use pxl_portable::area::edge;
use pxl_portable::ground::{blocks, coarser, read_cell, Cell};
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

/// The ground's cells as a reader holds them (pxl-kernel
/// layer/terrain/window.rs).
pub trait Cells {
    /// The cell `g` at zoom `cell`, off the finest window holding it present
    /// (pxl-portable `window::find`): its four looks, its height and the zoom
    /// of the cell the word is, a coarser window's where one stands in;
    /// `void` at sea level where none does.
    fn cell(&self, cell: Zoom, g: [i32; 2]) -> (u32, i32, Zoom);
}

pub struct Around<C> {
    pub cells: C,
    /// The asked tile's first cell, at its own cells' zoom `zoom`.
    pub first: [i64; 2],
    pub zoom: Zoom,
    /// Whether a point, in units, is past the service area: the one
    /// judgement (pxl-kernel io/source.rs `past_at`), asked of every cell
    /// whatever tile it is read from, or none, since a tile past the area is
    /// never held.
    pub past: fn([f64; 2]) -> bool,
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

/// A window of the ground's grid at one zoom, each cell a block shifted up
/// to its height and what its top block shows. Coarser than the leaves a
/// cell folds the cells under it in 2D, its height their mean, its look
/// their majority and its color theirs (`fold`).
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
        Ok(())
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

    /// Cell `i`'s ground as the one rule takes it (pxl-portable `Cell`).
    fn cell(&self, i: usize) -> Cell {
        Cell {
            look: u32::from(self.face[i]),
            top: self.top[i],
            fold: self.fold[i].map(u32::from),
        }
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
}

impl Grid {
    /// The column `c` counted from the first, as an index, if held.
    fn local(&self, c: [i32; 2]) -> Option<usize> {
        let span = self.span as i32;
        ((0..span).contains(&c[0]) && (0..span).contains(&c[1])).then(|| (c[1] * span + c[0]) as usize)
    }
}
