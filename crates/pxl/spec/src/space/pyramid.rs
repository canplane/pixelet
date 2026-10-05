// pxl-spec/space/pyramid.rs

//! The pyramid: tiles of a tile grid, from the leaves at z15 up to z5, a
//! tile's zoom its size's (`zoom.rs`), so 2^z of them stand around the world;
//! a layer's cell is its tile's side over the layer's own count of cells,
//! the same at every zoom, so its cells' zoom is its tile's plus a constant. Cell placement, tile membership,
//! folding order and cell centers are decided here only; a layer owns what a
//! cell carries and how it folds (`Folds`).

use super::hierarchy::{ancestor_of, child_of, first_descendant_of, parent_of, slot_of};
use super::indexer::idx;
use super::pvf3::core::FRAC_W;
use super::pvf3::ivec::{self, IVec};
use super::pvf3::pvf3::{self, Pvf3};
use super::pvf3::{projection, REACH};
use super::v3::Aabb;
use super::xy::TO_XY;
use super::zoom::{base_of, count, index_of, shamt, units, Zoom, FINEST_Z};
use crate::bake::constant::{CELL, DECK_CELL, EXTENT_CELL};

/// The leaves' zoom: a ground tile is 512 cells a side.
pub const LEAF_Z: Zoom = CELL - 9;

/// The coarsest folded zoom: where the pyramid stops for now, not a limit. A
/// coarser one extends it.
pub const TOP_Z: Zoom = 5;

/// A chunk is 64 cells a side: its zoom is its cells' less this.
pub const CHUNK_STEPS: Zoom = 6;

/// A layer's lattice: its cell is its tile's side over its own count of
/// cells, the same at every zoom; held as the cell's zoom on a leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    cell: Zoom,
}

impl Lattice {
    /// Finer than a point resolves, or coarser than a leaf's chunk, holds no tile.
    pub const fn new(cell: Zoom) -> Lattice {
        if cell >= FINEST_Z || cell < CELL - CHUNK_STEPS {
            panic!()
        }
        Lattice { cell }
    }

    /// Its cells' zoom on a tile at `z`.
    pub const fn cell_zoom(self, z: Zoom) -> Zoom {
        z + self.tile_cell_shamt() as Zoom
    }

    /// The tile zoom whose cells are at `cell_zoom`, past the pyramid's either end alike.
    pub const fn z_of(self, cell_zoom: Zoom) -> i32 {
        cell_zoom as i32 - self.tile_cell_shamt() as i32
    }

    /// `tile_cells` as a shift.
    pub const fn tile_cell_shamt(self) -> u32 {
        (self.cell - LEAF_Z) as u32
    }

    /// How many of its finest cells stand across one at `z`.
    pub const fn finest_in(self, z: Zoom) -> u32 {
        count(self.cell, self.cell_zoom(z))
    }

    pub const fn tile_cells(self) -> u32 {
        count(self.cell, LEAF_Z)
    }

    pub const fn chunk_cells(self) -> u32 {
        count(self.cell, chunk_zoom(LEAF_Z))
    }

    pub const fn cell_index(self, tx: u32, ty: u32) -> u32 {
        ty * self.tile_cells() + tx
    }

    /// The cell at `z` a point stands in; a point, since a cell may be finer than a unit.
    pub fn cell_of(self, z: Zoom, at: &Pvf3) -> Cell {
        let tile = Tile::over(z, idx(LEAF_Z, at.i));
        let [x, y, _] = index_of(self.cell_zoom(z), at);
        let [x0, y0] = first_descendant_of([ivec::X(tile.tile), ivec::Y(tile.tile)], self.tile_cell_shamt());
        Cell {
            lattice: self,
            tile,
            index: self.cell_index((x - x0) as u32, (y - y0) as u32),
        }
    }

    pub fn cell_at(self, tile: Tile, index: u32) -> Cell {
        xpute_core::ensure!(index < self.tile_cells() * self.tile_cells(), ENOTRECOVERABLE);
        Cell { lattice: self, tile, index }
    }

    pub fn cell_base(self, tile: Tile, tx: u32, ty: u32) -> Pvf3 {
        let [x0, y0] = first_descendant_of([ivec::X(tile.tile), ivec::Y(tile.tile)], self.tile_cell_shamt());
        base_of(self.cell_zoom(tile.zoom), [x0 + i64::from(tx), y0 + i64::from(ty), 0])
    }

    // A tile's four children form one square of 2n × 2n cells a zoom finer, slot q at (q & 1, q >> 1).

    pub fn parent_cell(self, q: usize, index: u32) -> u32 {
        let n = self.tile_cells() as i64;
        let (tile, [tx, ty]) = (child_of([0, 0], q), self.in_tile(index).map(i64::from));
        let [x, y] = parent_of([tile[0] * n + tx, tile[1] * n + ty]);
        self.cell_index(x as u32, y as u32)
    }

    /// The child slot and the 2×2 cells, in `slot_of`'s order, a parent cell
    /// folds from: `parent_cell`'s inverse.
    pub fn child_cells(self, index: u32) -> (usize, [u32; 4]) {
        let n = self.tile_cells() as i64;
        let [x, y] = child_of(self.in_tile(index).map(i64::from), 0);
        let [x0, y0] = [(x % n) as u32, (y % n) as u32];
        let cells = std::array::from_fn(|q| self.cell_index(x0 + (q & 1) as u32, y0 + (q >> 1) as u32));
        (slot_of([x / n, y / n]), cells)
    }

    /// The cells of this lattice under `outer`'s cell `index` of the same tile, row by row.
    pub fn under(self, outer: Lattice, index: u32) -> impl Iterator<Item = u32> {
        let n = count(self.cell, outer.cell);
        let [x0, y0] = outer.in_tile(index).map(|v| v * n);
        (0..n).flat_map(move |dy| (0..n).map(move |dx| self.cell_index(x0 + dx, y0 + dy)))
    }

    pub const fn in_tile(self, index: u32) -> [u32; 2] {
        let n = self.tile_cells();
        [index % n, index / n]
    }

    // A tile's coarser cells, j zooms up for j from 1, finest first, each zoom row by row.

    pub const fn coarse_cells(self) -> u32 {
        self.first_coarse(self.tile_cell_shamt() + 1)
    }

    pub const fn coarse_index(self, j: u32, tx: u32, ty: u32) -> u32 {
        if j == 0 || j > self.tile_cell_shamt() {
            panic!()
        }
        self.first_coarse(j) + (ty >> j) * (self.tile_cells() >> j) + (tx >> j)
    }

    const fn first_coarse(self, j: u32) -> u32 {
        let mut first = 0;
        let mut k = 1;
        while k < j {
            let side = self.tile_cells() >> k;
            first += side * side;
            k += 1;
        }
        first
    }
}

pub const GROUND: Lattice = Lattice::new(CELL);

/// The index lane's lattice (bake/format.rs `Index`).
pub const EXTENT: Lattice = Lattice::new(EXTENT_CELL);

pub const DECK: Lattice = Lattice::new(DECK_CELL);

pub const TILE_CELLS: u32 = GROUND.tile_cells();

pub const CHUNK_CELLS: u32 = GROUND.chunk_cells();

pub const TILE_CHUNKS: u32 = TILE_CELLS / CHUNK_CELLS;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tile {
    /// Its size's.
    pub zoom: Zoom,
    /// Z is zero.
    pub tile: IVec,
}

impl Tile {
    pub fn new(z: Zoom, tile: IVec) -> Tile {
        Tile { zoom: z, tile: TO_XY(tile) }
    }

    /// The tile at `z` a leaf lies under.
    pub fn over(z: Zoom, leaf: IVec) -> Tile {
        let [x, y] = ancestor_of([ivec::X(leaf), ivec::Y(leaf)], u32::from(LEAF_Z - z));
        Tile::new(z, ivec::gather(x, y, 0))
    }

    fn at(self) -> [i64; 2] {
        [ivec::X(self.tile), ivec::Y(self.tile)]
    }

    pub fn parent(self) -> Tile {
        let [x, y] = parent_of(self.at());
        Tile::new(self.zoom - 1, ivec::gather(x, y, 0))
    }

    /// In `slot_of`'s order.
    pub fn children(self) -> [Tile; 4] {
        std::array::from_fn(|slot| {
            let [x, y] = child_of(self.at(), slot);
            Tile::new(self.zoom + 1, ivec::gather(x, y, 0))
        })
    }

    pub fn node(self) -> (Zoom, [i64; 2]) {
        (self.zoom, self.at())
    }

    /// Its first leaf.
    pub fn first_leaf(self) -> IVec {
        let [x, y] = first_descendant_of(self.at(), u32::from(LEAF_Z - self.zoom));
        ivec::gather(x, y, 0)
    }

    pub fn span(self) -> f64 {
        units(self.zoom)
    }

    /// South-west corner and side, in units.
    pub fn rect(self) -> (f64, f64, f64) {
        let (z, [x, y]) = self.node();
        let base = base_of(z, [x, y, 0]);
        (pvf3::to_real(pvf3::X(&base)), pvf3::to_real(pvf3::Y(&base)), units(z))
    }

    /// The tile as a box of the world: its square, every height.
    pub fn region(self) -> Aabb {
        let (x0, y0, side) = self.rect();
        Aabb {
            min: [x0, y0, REACH.min[2]],
            max: [x0 + side, y0 + side, REACH.max[2]],
        }
    }

    /// A demand's subject: the key with the zoom in the Z lane.
    pub fn subject(self) -> u64 {
        ivec::gather(ivec::X(self.tile), ivec::Y(self.tile), self.zoom as i64)
    }

    pub fn of_subject(subject: u64) -> Tile {
        Tile::new(ivec::Z(subject) as Zoom, subject)
    }
}

/// A ground chunk's zoom on a tile at `z`.
pub const fn chunk_zoom(z: Zoom) -> Zoom {
    GROUND.cell_zoom(z) - CHUNK_STEPS
}

/// The 4×4 tiles a zoom finer around a tile's children, each at
/// `neighbor_index`; X wraps and a tile past Y's edge has no object.
pub fn neighborhood(p: Tile) -> [Tile; 16] {
    let [x, y] = child_of(p.at(), 0);
    std::array::from_fn(|k| {
        let (qx, qy) = ((k % 4) as i64 - 1, (k / 4) as i64 - 1);
        tile_at(p.zoom + 1, [x + qx, y + qy])
    })
}

/// Where the tile (qx, qy) from a parent's first child, each −1 to 2, lies in
/// its `neighborhood`.
pub const fn neighbor_index(qx: i64, qy: i64) -> usize {
    ((qx + 1) + 4 * (qy + 1)) as usize
}

/// How many cells at `z` stand around the world, and as many from its south edge to its north.
pub const fn around(z: Zoom) -> i64 {
    1i64 << z
}

/// The rows of tiles at `z`, south to north; past them is no tile.
pub fn tile_rows(z: Zoom) -> std::ops::Range<i64> {
    let n = around(z);
    -n / 2..n / 2
}

/// An X index of `z`'s cells as the lane holds it; every key goes through it.
pub fn wrap_index(z: Zoom, x: i64) -> i64 {
    let across = around(z);
    (x + across / 2).rem_euclid(across) - across / 2
}

/// A cell at `z` named as `wrap_index` names its X.
pub fn wrap_cell(z: Zoom, [x, y, h]: [i64; 3]) -> [i64; 3] {
    [wrap_index(z, x), y, h]
}

/// The tile at `z` at (x, y), X wrapped around the world.
pub fn tile_at(z: Zoom, [x, y]: [i64; 2]) -> Tile {
    Tile::new(z, ivec::gather(wrap_index(z, x), y, 0))
}

/// Where a tile's own children sit in its neighborhood, in `slot_of`'s order.
pub const CHILDREN: [usize; 4] = [5, 6, 9, 10];

/// A cell of the pyramid: lattice, tile and index. Built only from a point
/// or a tile's own index, so a mismatched zoom and index cannot be held.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cell {
    lattice: Lattice,
    tile: Tile,
    index: u32,
}

impl Cell {
    pub fn lattice(self) -> Lattice {
        self.lattice
    }

    pub fn tile(self) -> Tile {
        self.tile
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn tile_zoom(self) -> Zoom {
        self.tile.zoom
    }

    /// Its own.
    pub fn zoom(self) -> Zoom {
        self.lattice.cell_zoom(self.tile.zoom)
    }

    pub fn in_tile(self) -> [u32; 2] {
        self.lattice.in_tile(self.index)
    }

    pub fn base(self) -> Pvf3 {
        let [tx, ty] = self.in_tile();
        self.lattice.cell_base(self.tile, tx, ty)
    }

    pub fn mid(self) -> Pvf3 {
        let base = self.base();
        let half = 1i64 << (FRAC_W as i32 + shamt(self.zoom()) - 1);
        pvf3::gather(pvf3::X(&base) + half, pvf3::Y(&base) + half, 0)
    }

    pub fn side(self) -> f64 {
        units(self.zoom())
    }

    /// The cell `steps` zooms coarser it lies in.
    pub fn up(self, steps: Zoom) -> Cell {
        self.lattice.cell_of(self.tile.zoom - steps, &self.base())
    }

    /// Where `inner`, no coarser and inside this cell, lies in it, in `inner`'s sides.
    pub fn offset_of(self, inner: Cell) -> [u32; 2] {
        let (outer, inner_base) = (self.base(), inner.base());
        let shift = FRAC_W as i32 + shamt(inner.zoom());
        let lane = |o: i64, i: i64| ((i - o) >> shift) as u32;
        [lane(pvf3::X(&outer), pvf3::X(&inner_base)), lane(pvf3::Y(&outer), pvf3::Y(&inner_base))]
    }

    /// Where this cell lies in the tile `steps` zooms coarser, in its own sides.
    pub fn in_tile_above(self, steps: Zoom) -> [i64; 2] {
        let up = self.up(steps);
        let [ax, ay] = up.in_tile();
        let n = count(self.zoom(), up.zoom()) as i64;
        let [ox, oy] = up.offset_of(self);
        [ax as i64 * n + ox as i64, ay as i64 * n + oy as i64]
    }

    /// `offset_of`'s inverse.
    pub fn inner_at(self, lattice: Lattice, z: Zoom, offset: [u32; 2]) -> Cell {
        let base = self.base();
        let shift = FRAC_W as i32 + shamt(lattice.cell_zoom(z));
        let at = pvf3::gather(pvf3::X(&base) + ((offset[0] as i64) << shift), pvf3::Y(&base) + ((offset[1] as i64) << shift), 0);
        lattice.cell_of(z, &at)
    }

    /// The point every layer stands a thing at: the XY center, half a side above
    /// `ground`, since a zoom's cells are as tall as they are wide.
    pub fn center(self, ground: f64) -> Pvf3 {
        let flat = self.mid();
        let at = projection::unproject(flat.i, flat.f);
        projection::project(at[0], at[1], ground + self.side() / 2.0)
    }
}

/// A layer's tile as the pyramid holds it. One fold makes each zoom's tiles
/// from the next finer one's, run by the bake and by the client. A parent reads its four
/// children unless `sources` says otherwise.
pub trait Folds: Sized {
    fn sources(p: Tile) -> Vec<Tile> {
        p.children().to_vec()
    }

    /// `p` from its sources; None where a required one is not held.
    fn fold_tile<'a>(p: Tile, held: impl Fn(Tile) -> Option<&'a Self>) -> Option<Self>
    where
        Self: 'a;

    /// Whether `p` already says all of `child`'s ground, so the tree stops
    /// there and the child is never a tile of its own.
    fn settles(&self, _p: Tile, _child: Tile) -> bool {
        false
    }
}

#[cfg(test)]
#[path = "pyramid.test.rs"]
mod test;
