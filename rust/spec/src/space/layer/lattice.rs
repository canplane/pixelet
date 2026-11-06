// pxl-spec/space/layer/lattice.rs

//! A layer's lattice: how many cells its tile holds a side, the same at
//! every zoom, and its scale on the camera grid's cells.

use super::quadrant::{Cell, Quadrant, TOP_ZOOM};
use crate::space::camera::grid::{cell_zoom_at, split_dist, CLIP_MARGIN};
use crate::space::coord::q3::Q3;
use crate::space::coord::zoom::{idx_of, Zoom};
use crate::space::hierarchy::tree::{
    ancestor_of, child_idx, child_of, first_descendant_of, parent_of,
};
use pxl_engine::cast::REACH;

/// A lattice's integer core — its zooms, its cells' indices, the ground's —
/// is pxl-engine's, which a GPU reads tiles by too; placing it in the
/// world is here (`Cells`).
pub use pxl_engine::lattice::{Lattice, GROUND, TILE_CELLS};

/// The coarsest zoom `lattice`'s tiles are read at: where it reads the top's
/// ground cells, the coarsest the camera grid draws. A tile coarser is one no
/// reader asks for, so the bake folds no coarser.
pub const fn coarsest(lattice: Lattice) -> Zoom {
    lattice.read_zoom(GROUND.cell_zoom(TOP_ZOOM))
}

/// How far from the eye the view draws with `zoom` its coarsest:
/// where the grid asks the cell a zoom coarser, which nothing holds, so a ray
/// past it would cross cells finer than its distance asks.
pub const fn draw_dist(lattice: Lattice, zoom: Zoom) -> f64 {
    split_dist(lattice.grid_zoom(zoom - 1))
}
/// Every effect's one reach from the eye: out to where the grid's cells one zoom coarser
/// than the ground's leaf begin, where the casters are drawn whole and a face's bits read the
/// leaf's (pxl-kernel domain/scene/stage.rs; pxl-render gpu/shadow.rs `half_width`).
pub const EFFECT_REACH: f64 = split_dist(GROUND.cell() - 1);

const _: () = assert!(
    draw_dist(GROUND, TOP_ZOOM) < REACH as f64,
    "the eye draws within the reach every node it stops on is named and kept by"
);

/// How far from an observer's foot a demand at `zoom` can lie: where a
/// clip level holds the grid's cell a zoom coarser from (`CLIP_MARGIN`), as
/// far past `draw_dist` as it holds every finer cell past where it is
/// drawn. Bounds the run along the ground too.
pub const fn demand_dist(lattice: Lattice, zoom: Zoom) -> f64 {
    draw_dist(lattice, zoom) * CLIP_MARGIN
}

/// A lattice placed in the world: the tiles and cells a node, a point or an
/// index names.
pub trait Cells {
    fn read_tile(self, node: (Zoom, [i32; 2]), cell: Zoom) -> Quadrant;
    fn z_at(self, r2: f64) -> Zoom;
    fn cell_of(self, zoom: Zoom, at: &Q3) -> Cell;
    fn cell_at(self, tile: Quadrant, idx: u32) -> Cell;
    fn parent_cell(self, q: usize, idx: u32) -> u32;
    fn child_cells(self, idx: u32) -> (usize, [u32; 4]);
}

impl Cells for Lattice {
    fn read_tile(self, (zoom, column): (Zoom, [i32; 2]), cell: Zoom) -> Quadrant {
        let z = self.read_zoom(cell);
        Quadrant::new(z, ancestor_of(column, zoom - z))
    }

    /// The tile whose records fold into the grid's cell at `r2`, a distance
    /// squared; held to the pyramid.
    fn z_at(self, r2: f64) -> Zoom {
        self.read_zoom(cell_zoom_at(r2))
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

    fn cell_at(self, tile: Quadrant, idx: u32) -> Cell {
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

    /// The child's place and the 2×2 cells, in `child_idx`'s order, a parent cell
    /// folds from: `parent_cell`'s inverse.
    fn child_cells(self, idx: u32) -> (usize, [u32; 4]) {
        let n = self.tile_cells() as i32;
        let [x, y] = child_of(self.in_tile(idx).map(|v| v as i32), 0);
        let [x0, y0] = [(x % n) as u32, (y % n) as u32];
        let cells =
            std::array::from_fn(|q| self.cell_idx(x0 + (q & 1) as u32, y0 + (q >> 1) as u32));
        (child_idx([x / n, y / n]), cells)
    }
}
