// pxl-spec/space/layer/lattice.rs

//! A layer's lattice: how many cells its tile holds a side, the same at
//! every zoom, and its scale on the camera grid's cells.

use super::tile::{tile_at, Cell, Tile};
use crate::space::camera::grid::{cell_at, split_dist, WINDOW_MARGIN};
use crate::space::coord::q3::Q3;
use crate::space::coord::zoom::{idx_of, Zoom};
use crate::space::hierarchy::tree::{
    ancestor_of, child_of, first_descendant_of, parent_of, slot_of,
};

/// A lattice's integer core — its zooms, its cells' indices, the ground's —
/// is pxl-spec-core's, which a GPU reads tiles by too; placing it in the
/// world is here (`Cells`).
pub use pxl_spec_core::lattice::{Lattice, GROUND, TILE_CELLS};

/// The coarsest zoom `lattice`'s tiles are read at: where it reads the top's
/// ground cells, the coarsest the camera grid draws. A tile coarser is one no
/// reader asks for, so the bake folds no coarser.
pub const fn coarsest(lattice: Lattice) -> Zoom {
    lattice.read_zoom(GROUND.cell_zoom(super::tile::TOP_ZOOM))
}

/// How far from the eye the view draws with `zoom` its coarsest:
/// where the grid asks the cell a zoom coarser, which nothing holds, so a ray
/// past it would cross cells finer than its distance asks.
pub const fn draw_reach(lattice: Lattice, zoom: Zoom) -> f64 {
    split_distance(lattice.grid_zoom(zoom - 1))
}

/// How far from an observer's foot a demand at `zoom` can lie: where a
/// window holds the grid's cell a zoom coarser from (`WINDOW_MARGIN`), as
/// far past `draw_reach` as it holds every finer cell past where it is
/// drawn. Bounds the run along the ground too.
pub const fn demand_reach(lattice: Lattice, zoom: Zoom) -> f64 {
    draw_reach(lattice, zoom) * WINDOW_MARGIN
}

/// A lattice placed in the world: the tiles and cells a node, a point or an
/// index names.
pub trait Cells {
    fn read_tile(self, node: (Zoom, [i32; 2]), cell: Zoom) -> Tile;
    fn z_at(self, r2: f64) -> Zoom;
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell;
    fn cell_at(self, tile: Tile, idx: u32) -> Cell;
    fn parent_cell(self, q: usize, idx: u32) -> u32;
    fn child_cells(self, idx: u32) -> (usize, [u32; 4]);
}

impl Cells for Lattice {
    fn read_tile(self, (zoom, column): (Zoom, [i32; 2]), cell: Zoom) -> Tile {
        let z = self.read_zoom(cell);
        tile_at(z, ancestor_of(column, zoom - z))
    }

    /// The tile whose records fold into the grid's cell at `r2`, a distance
    /// squared; held to the pyramid.
    fn z_at(self, r2: f64) -> Zoom {
        self.read_zoom(cell_at(r2))
    }

    /// The cell at `z` a point stands in; a point, since a cell may be finer than a unit.
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell {
        let zoom = self.cell_zoom(zoom);
        let [x, y, _] = idx_of(zoom, at);
        Cell {
            lattice: self,
            zoom,
            at: [x, y],
        }
    }

    fn cell_at(self, tile: Tile, idx: u32) -> Cell {
        xpute_kit::ensure!(idx < self.tile_cells() * self.tile_cells(), ENOTRECOVERABLE);
        let (zoom, at) = tile.node();
        let [x0, y0] = first_descendant_of(at, self.tile_cell_shamt());
        let [tx, ty] = self.in_tile(idx);
        Cell {
            lattice: self,
            zoom: self.cell_zoom(zoom),
            at: [x0 + tx as i32, y0 + ty as i32],
        }
    }

    // A tile's four children form one square of 2n × 2n cells a zoom finer, slot q at (q & 1, q >> 1).

    fn parent_cell(self, q: usize, idx: u32) -> u32 {
        let n = self.tile_cells() as i32;
        let (tile, [tx, ty]) = (child_of([0, 0], q), self.in_tile(idx).map(|v| v as i32));
        let [x, y] = parent_of([tile[0] * n + tx, tile[1] * n + ty]);
        self.cell_idx(x as u32, y as u32)
    }

    /// The child slot and the 2×2 cells, in `slot_of`'s order, a parent cell
    /// folds from: `parent_cell`'s inverse.
    fn child_cells(self, idx: u32) -> (usize, [u32; 4]) {
        let n = self.tile_cells() as i32;
        let [x, y] = child_of(self.in_tile(idx).map(|v| v as i32), 0);
        let [x0, y0] = [(x % n) as u32, (y % n) as u32];
        let cells =
            std::array::from_fn(|q| self.cell_idx(x0 + (q & 1) as u32, y0 + (q >> 1) as u32));
        (slot_of([x / n, y / n]), cells)
    }
}
