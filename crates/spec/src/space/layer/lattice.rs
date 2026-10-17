// pxl-spec/space/layer/lattice.rs

//! A layer's lattice: how many cells its tile holds a side, the same at
//! every zoom, and its scale on the camera grid's cells.

use super::tile::{tile_at, tile_rows, Cell, Tile};
use crate::space::camera::grid::{split_distance, Tau};
use crate::space::coord::q3::Q3;
use crate::space::coord::zoom::{index_of, Zoom};
use crate::space::hierarchy::tree::{ancestor_of, child_of, first_descendant_of, offset_in, parent_of, slot_of};

/// A lattice's integer core — its zooms, its cells' indices, the ground's
/// and the index lane's — is pxl-portable's, which a GPU reads tiles by
/// too; placing it in the world is here (`Cells`).
pub use pxl_portable::lattice::{Lattice, GROUND, TILE_CELLS};

/// How far from an observer's foot a demand at `zoom` can lie: where the
/// grid's cell a zoom coarser begins. Bounds the run along the ground too.
pub const fn demand_reach(lattice: Lattice, zoom: Zoom) -> f64 {
    split_distance(lattice.grid_zoom(zoom - 1))
}

/// A lattice placed in the world: the tiles and cells a node, a point or an
/// index names.
pub trait Cells {
    fn read_tile(self, node: (Zoom, [i64; 2]), cell: Zoom) -> Tile;
    fn read_at(self, node: (Zoom, [i64; 2]), cell: Zoom) -> (u32, u32);
    fn read_beside(self, node: (Zoom, [i64; 2]), cell: Zoom, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)>;
    fn z_at(self, tau: Tau, r: f64) -> Zoom;
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell;
    fn cell_at(self, tile: Tile, index: u32) -> Cell;
    fn parent_cell(self, q: usize, index: u32) -> u32;
    fn child_cells(self, index: u32) -> (usize, [u32; 4]);
}

impl Cells for Lattice {
    fn read_tile(self, (zoom, column): (Zoom, [i64; 2]), cell: Zoom) -> Tile {
        let z = self.read_zoom(cell);
        tile_at(z, ancestor_of(column, zoom - z))
    }

    /// The node's first cell's place in the tile it reads, in that tile's cells.
    fn read_at(self, (zoom, column): (Zoom, [i64; 2]), cell: Zoom) -> (u32, u32) {
        let [x, y] = offset_in(column, zoom - self.read_zoom(cell));
        let side = self.read_side(zoom, cell);
        (x as u32 * side, y as u32 * side)
    }

    /// Neighbouring tiles an apron of `apron` of the tile's cells reaches
    /// into, with their offsets from the node's tile; none for a node inside it.
    fn read_beside(self, node: (Zoom, [i64; 2]), cell: Zoom, apron: u32) -> impl Iterator<Item = ([i64; 2], Tile)> {
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
    fn z_at(self, tau: Tau, r: f64) -> Zoom {
        self.read_zoom(tau.cell_at(r))
    }

    /// The cell at `z` a point stands in; a point, since a cell may be finer than a unit.
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell {
        let zoom = self.cell_zoom(zoom);
        let [x, y, _] = index_of(zoom, at);
        Cell { lattice: self, zoom, at: [x, y] }
    }

    fn cell_at(self, tile: Tile, index: u32) -> Cell {
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

    fn parent_cell(self, q: usize, index: u32) -> u32 {
        let n = self.tile_cells() as i64;
        let (tile, [tx, ty]) = (child_of([0, 0], q), self.in_tile(index).map(i64::from));
        let [x, y] = parent_of([tile[0] * n + tx, tile[1] * n + ty]);
        self.cell_index(x as u32, y as u32)
    }

    /// The child slot and the 2×2 cells, in `slot_of`'s order, a parent cell
    /// folds from: `parent_cell`'s inverse.
    fn child_cells(self, index: u32) -> (usize, [u32; 4]) {
        let n = self.tile_cells() as i64;
        let [x, y] = child_of(self.in_tile(index).map(i64::from), 0);
        let [x0, y0] = [(x % n) as u32, (y % n) as u32];
        let cells = std::array::from_fn(|q| self.cell_index(x0 + (q & 1) as u32, y0 + (q >> 1) as u32));
        (slot_of([x / n, y / n]), cells)
    }
}
