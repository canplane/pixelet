// pxl-spec/space/layer/lattice.rs

//! A layer's lattice: how many cells its tile holds a side, the same at
//! every zoom, and which of the camera grid's cells they fold into.

use super::tile::{tile_at, tile_rows, Cell, Tile, LEAF_ZOOM};
use crate::bake::constant::{DECK_CELL, EXTENT_CELL};
use crate::space::camera::grid::{split_distance, Tau, CHUNK_CELLS, CHUNK_STEPS};
use crate::space::coord::pvf3::ivec;
use crate::space::coord::pvf3::Pvf3;
use crate::space::coord::zoom::{base_of, count, idx, index_of, Zoom, FINEST_ZOOM, SHARD_ZOOM};
use crate::space::hierarchy::tree::{ancestor_of, child_of, first_descendant_of, offset_in, parent_of, slot_of};

/// A layer's lattice: its cell is its tile's side over its own count of
/// cells, the same at every zoom; held as the cell's zoom on a leaf. The
/// camera grid's cell is drawn from its cells `fold` zooms finer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    cell: Zoom,
    fold: Zoom,
}

impl Lattice {
    /// Finer than a point resolves, or no finer than the leaf tile, holds no tile.
    pub const fn new(cell: Zoom) -> Lattice {
        if cell >= FINEST_ZOOM || cell <= LEAF_ZOOM {
            panic!()
        }
        Lattice { cell, fold: 0 }
    }

    /// Drawn in the camera grid's cells `fold` zooms coarser than its own.
    pub const fn folding(self, fold: Zoom) -> Lattice {
        Lattice { fold, ..self }
    }

    pub const fn fold(self) -> Zoom {
        self.fold
    }

    /// The coarsest tile it is kept in: the one whose cells fold into the
    /// coarsest grid cell any chunk has, a 3D root's (`SHARD_ZOOM`) cells. The
    /// ground's lands on z5 exactly: z8's chunks have z13 cells, which the
    /// ground's 512 a tile and one fold make a z5 tile's.
    pub const fn top(self) -> Zoom {
        self.grid_z(SHARD_ZOOM + CHUNK_STEPS) as Zoom
    }

    // What a layer reads for a chunk, named by its zoom and column: the tile
    // whose cells fold into the chunk's (`CHUNK_STEPS` finer), or past the
    // leaves its leaf, and where the chunk lies in it.

    /// The zoom of the tile a chunk at `chunk_zoom` reads.
    pub const fn read_zoom(self, chunk_zoom: Zoom) -> Zoom {
        let z = self.grid_z(chunk_zoom + CHUNK_STEPS);
        (if z < LEAF_ZOOM as i32 { z } else { LEAF_ZOOM as i32 }) as Zoom
    }

    /// A chunk's side in the cells of the tile it reads.
    pub const fn read_side(self, chunk_zoom: Zoom) -> u32 {
        self.tile_cells() >> (chunk_zoom - self.read_zoom(chunk_zoom))
    }

    pub fn read_tile(self, (zoom, column): (Zoom, [i64; 2])) -> Tile {
        let z = self.read_zoom(zoom);
        tile_at(z, ancestor_of(column, u32::from(zoom - z)))
    }

    /// The chunk's first cell's place in the tile it reads, in that tile's cells.
    pub fn read_at(self, (zoom, column): (Zoom, [i64; 2])) -> (u32, u32) {
        let [x, y] = offset_in(column, u32::from(zoom - self.read_zoom(zoom)));
        let side = self.read_side(zoom);
        (x as u32 * side, y as u32 * side)
    }

    /// Neighbouring tiles an apron of `apron` of the tile's cells reaches
    /// into, with their offsets from the chunk's tile; none for a chunk inside it.
    pub fn read_beside(self, chunk: (Zoom, [i64; 2]), apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.read_at(chunk);
        let (side, across) = (self.read_side(chunk.0), self.tile_cells());
        let steps = move |v: u32| [0, -1, 1].into_iter().filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + side + apron > across));
        let tile = self.read_tile(chunk);
        let [px, py] = [ivec::X(tile.tile), ivec::Y(tile.tile)];
        let zoom = tile.zoom;
        let rows = tile_rows(zoom);
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(move |&[qx, qy]| [qx, qy] != [0, 0] && rows.contains(&(py + qy)))
            .map(move |[qx, qy]| ([qx, qy], tile_at(zoom, [px + qx, py + qy])))
    }

    /// Its finest cell's zoom: its cells' on a leaf tile.
    pub const fn cell(self) -> Zoom {
        self.cell
    }

    /// The grid's cell a tile at `zoom` is drawn in.
    pub const fn grid_cell(self, zoom: Zoom) -> Zoom {
        self.cell_zoom(zoom) - self.fold
    }

    /// The tile whose cells fold into the grid's cell `cell`, past the pyramid's ends too.
    pub const fn grid_z(self, cell: Zoom) -> i32 {
        self.z_of(cell) + self.fold as i32
    }

    /// The tile whose records fold into the grid's cell at `r`; held to the pyramid.
    pub fn z_at(self, tau: Tau, r: f64) -> Zoom {
        self.grid_z(tau.cell_at(r)).clamp(self.top() as i32, LEAF_ZOOM as i32) as Zoom
    }

    /// How far from an observer's foot a demand at `zoom` can lie: where the
    /// grid's cell a zoom coarser begins. Bounds the run along the ground too.
    pub const fn demand_reach(self, zoom: Zoom) -> f64 {
        split_distance(self.grid_cell(zoom - 1))
    }

    /// Its cells' zoom on a tile at `z`.
    pub const fn cell_zoom(self, zoom: Zoom) -> Zoom {
        zoom + self.tile_cell_shamt() as Zoom
    }

    /// The tile zoom whose cells are at `cell_zoom`, past the pyramid's either end alike.
    pub const fn z_of(self, cell_zoom: Zoom) -> i32 {
        cell_zoom as i32 - self.tile_cell_shamt() as i32
    }

    /// `tile_cells` as a shift.
    pub const fn tile_cell_shamt(self) -> u32 {
        (self.cell - LEAF_ZOOM) as u32
    }

    /// How many of its finest cells stand across one at `z`.
    pub const fn finest_in(self, zoom: Zoom) -> u32 {
        count(self.cell, self.cell_zoom(zoom))
    }

    pub const fn tile_cells(self) -> u32 {
        count(self.cell, LEAF_ZOOM)
    }

    pub const fn cell_index(self, tx: u32, ty: u32) -> u32 {
        ty * self.tile_cells() + tx
    }

    /// The cell at `z` a point stands in; a point, since a cell may be finer than a unit.
    pub fn cell_of(self, zoom: Zoom, at: &Pvf3) -> Cell {
        let tile = Tile::over(zoom, idx(LEAF_ZOOM, at.i));
        let [x, y, _] = index_of(self.cell_zoom(zoom), at);
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

/// The ground's tile is 512 cells a side, its one knob: every length the
/// ground keeps in cells follows it. A grid cell is drawn from the ground's
/// records a zoom finer: their surfaces' mean and their fill (pxl-render
/// mesh/chunk_mesher.rs).
pub const GROUND: Lattice = Lattice::new(LEAF_ZOOM + 9).folding(1);

/// The index lane's lattice (bake/format.rs `Index`).
pub const EXTENT: Lattice = Lattice::new(EXTENT_CELL);

pub const DECK: Lattice = Lattice::new(DECK_CELL);

pub const TILE_CELLS: u32 = GROUND.tile_cells();

/// Chunks a tile's side holds at the zoom whose cells fold it.
pub const TILE_CHUNKS: u32 = TILE_CELLS / (CHUNK_CELLS << GROUND.fold());
