// pxl-spec/space/layer/lattice.rs

//! A layer's lattice: how many cells its tile holds a side, the same at
//! every zoom, and its scale on the camera grid's cells.

use super::tile::{tile_at, tile_rows, Cell, Tile, LEAF_ZOOM};
use crate::bake::constant::EXTENT_CELL;
use crate::space::camera::grid::{split_distance, Tau, CHUNK_CELLS, CHUNK_STEPS};
use crate::space::coord::pvf3::Pvf3;
use crate::space::coord::zoom::{count, index_of, Zoom, FINEST_ZOOM, SHARD_ZOOM};
use crate::space::hierarchy::tree::{ancestor_of, child_of, first_descendant_of, offset_in, parent_of, slot_of};

/// A layer's lattice: its cell is its tile's side over its own count of
/// cells, the same at every zoom; held as the cell's zoom on a leaf. Its
/// scale on the camera grid is `scale_steps` zooms: the grid's cell is that
/// many zooms coarser than its own, so it folds that many of its cells a
/// side, or where negative, finer, so its cell spans that many of the grid's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lattice {
    cell: Zoom,
    scale_steps: i8,
}

impl Lattice {
    /// Finer than a point resolves, or no finer than the leaf tile, holds no tile.
    pub const fn new(cell: Zoom) -> Lattice {
        if cell >= FINEST_ZOOM || cell <= LEAF_ZOOM {
            panic!()
        }
        Lattice { cell, scale_steps: 0 }
    }

    /// Drawn in the camera grid's cells `steps` zooms coarser than its own,
    /// or where negative, finer.
    pub const fn scaled(self, steps: i8) -> Lattice {
        Lattice { scale_steps: steps, ..self }
    }

    pub const fn scale_steps(self) -> i8 {
        self.scale_steps
    }

    /// The coarsest tile it is kept in: the one whose cells fold into the
    /// coarsest grid cell any chunk has, a 3D root's (`SHARD_ZOOM`) cells. The
    /// ground's lands on z5 exactly: z8's chunks have z13 cells, which the
    /// ground's 512 a tile and its one step of scale make a z5 tile's.
    pub const fn top(self) -> Zoom {
        self.tile_zoom_of_grid(SHARD_ZOOM + CHUNK_STEPS) as Zoom
    }

    // What a layer reads for a node of the view, named by its zoom and column
    // and the cell zoom it is drawn at: the tile whose cells fold into those
    // cells, or past the leaves its leaf, and where the node lies in it.

    /// The zoom of the tile cells at `cell` are read from, held to the pyramid's zooms.
    pub const fn read_zoom(self, cell: Zoom) -> Zoom {
        let z = self.tile_zoom_of_grid(cell);
        let top = self.top() as i32;
        (if z < top {
            top
        } else if z < LEAF_ZOOM as i32 {
            z
        } else {
            LEAF_ZOOM as i32
        }) as Zoom
    }

    /// A node's side at `zoom` in the cells of the tile it reads.
    pub const fn read_side(self, zoom: Zoom, cell: Zoom) -> u32 {
        self.tile_cells() >> (zoom - self.read_zoom(cell))
    }

    pub fn read_tile(self, (zoom, column): (Zoom, [i64; 2]), cell: Zoom) -> Tile {
        let z = self.read_zoom(cell);
        tile_at(z, ancestor_of(column, u32::from(zoom - z)))
    }

    /// The node's first cell's place in the tile it reads, in that tile's cells.
    pub fn read_at(self, (zoom, column): (Zoom, [i64; 2]), cell: Zoom) -> (u32, u32) {
        let [x, y] = offset_in(column, u32::from(zoom - self.read_zoom(cell)));
        let side = self.read_side(zoom, cell);
        (x as u32 * side, y as u32 * side)
    }

    /// Neighbouring tiles an apron of `apron` of the tile's cells reaches
    /// into, with their offsets from the node's tile; none for a node inside it.
    pub fn read_beside(self, node: (Zoom, [i64; 2]), cell: Zoom, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
        let (x0, y0) = self.read_at(node, cell);
        let (side, across) = (self.read_side(node.0, cell), self.tile_cells());
        let steps = move |v: u32| [0, -1, 1].into_iter().filter(move |&q| q == 0 || (q < 0 && v < apron) || (q > 0 && v + side + apron > across));
        let tile = self.read_tile(node, cell);
        let [px, py] = tile.at;
        let zoom = tile.zoom;
        let rows = tile_rows(zoom);
        steps(y0)
            .flat_map(move |qy| steps(x0).map(move |qx| [qx, qy]))
            .filter(move |&[qx, qy]| [qx, qy] != [0, 0] && rows.contains(&(py + qy)))
            .map(move |[qx, qy]| ([qx, qy], tile_at(zoom, [px + qx, py + qy])))
    }

    /// The tile whose records fold into the grid's cell at `r`; held to the pyramid.
    pub fn z_at(self, tau: Tau, r: f64) -> Zoom {
        self.read_zoom(tau.cell_at(r))
    }

    /// How far from an observer's foot a demand at `zoom` can lie: where the
    /// grid's cell a zoom coarser begins. Bounds the run along the ground too.
    pub const fn demand_reach(self, zoom: Zoom) -> f64 {
        split_distance(self.grid_zoom(zoom - 1))
    }

    /// Its finest cell's zoom: its cells' on a leaf tile.
    pub const fn cell(self) -> Zoom {
        self.cell
    }

    /// The grid's cell a tile at `zoom` is drawn in.
    pub const fn grid_zoom(self, zoom: Zoom) -> Zoom {
        (self.cell_zoom(zoom) as i32 - self.scale_steps as i32) as Zoom
    }

    /// The tile whose cells fold into the grid's cell `cell`, past the pyramid's ends too.
    pub const fn tile_zoom_of_grid(self, cell: Zoom) -> i32 {
        self.tile_zoom(cell) + self.scale_steps as i32
    }

    /// Its cells' zoom on a tile at `z`.
    pub const fn cell_zoom(self, zoom: Zoom) -> Zoom {
        zoom + self.tile_cell_shamt() as Zoom
    }

    /// The tile zoom whose cells are at `cell_zoom`, past the pyramid's either end alike.
    pub const fn tile_zoom(self, cell_zoom: Zoom) -> i32 {
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
        let zoom = self.cell_zoom(zoom);
        let [x, y, _] = index_of(zoom, at);
        Cell { lattice: self, zoom, at: [x, y] }
    }

    pub fn cell_at(self, tile: Tile, index: u32) -> Cell {
        xpute_kit::ensure!(index < self.tile_cells() * self.tile_cells(), ENOTRECOVERABLE);
        let (zoom, at) = tile.node();
        let [x0, y0] = first_descendant_of(at, self.tile_cell_shamt());
        let [tx, ty] = self.in_tile(index);
        Cell {
            lattice: self,
            zoom: self.cell_zoom(zoom),
            at: [x0 + i64::from(tx), y0 + i64::from(ty)],
        }
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
/// mesh/chunk.rs).
pub const GROUND: Lattice = Lattice::new(LEAF_ZOOM + 9).scaled(1);

/// The zooms a grid cell folds of the ground's cells, which the chunk's runs
/// and the mesher are laid out by: the ground's scale, never finer than the grid.
pub const GROUND_FOLD: Zoom = GROUND.scale_steps() as Zoom;
const _: () = assert!(GROUND.scale_steps() >= 0, "the ground's cells are finer than the grid's");

/// The index lane's lattice (bake/tile/terrain.rs `Index`).
pub const EXTENT: Lattice = Lattice::new(EXTENT_CELL);

pub const TILE_CELLS: u32 = GROUND.tile_cells();

/// Chunks a tile's side holds at the zoom whose cells fold it.
pub const TILE_CHUNKS: u32 = TILE_CELLS / (CHUNK_CELLS << GROUND_FOLD);
