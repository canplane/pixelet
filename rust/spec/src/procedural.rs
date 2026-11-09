// pxl-spec/procedural.rs

//! What a tile's place in the world gives and no tile stores: where its bridge piers stand
//! (way.rs), and a ground meter in leaf cells. Pure functions of the world, so every reader
//! agrees.

use crate::space::coord::adapter::epsg3857::{cells_of_m, per_m};
use crate::space::coord::zoom::{units, LEAF_CELL_ZOOM};
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::quadrant::Quadrant;
use crate::way::{StationGrid, INTERVAL_M};

/// A tile's cells in the world, so a station is the world's and not a tile's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub corner: [f64; 2], // The tile's south-west corner, in units.
    pub cell: f64,        // A cell's side, in units.
}

impl Frame {
    pub fn of(tile: Quadrant) -> Frame {
        let (x0, y0, side) = tile.rect();
        Frame {
            corner: [x0, y0],
            cell: side / f64::from(TILE_CELLS),
        }
    }

    /// Leaf cells a ground meter over the tile, for height as for breadth: the scale where its
    /// south-west corner lies, one value a tile (3857.rs `per_m`). A leaf tile's is the
    /// scale by which the bake makes the ground's heights, and so a height's meters
    /// (3857.rs `height_u_per_m`).
    pub fn per_m(self) -> f32 {
        per_m(self.corner[1])
    }

    /// Where its piers may stand, in leaf cells (way.rs
    /// `StationGrid`): each beam's lattice of stations, at its spacing in
    /// ground meters where the tile lies, in whole leaf cells
    /// (`cells_of_m`), and where its corner falls in it; and the leaf cells
    /// its cell spans.
    pub(crate) fn stations(self) -> StationGrid {
        let leaf = units(LEAF_CELL_ZOOM);
        let corner = self.corner.map(|v| (v / leaf).round_ties_even() as i64);
        let step = INTERVAL_M.map(|m| cells_of_m(m, self.per_m()).max(1));
        StationGrid {
            step,
            phase: step.map(|s| corner.map(|c| c.rem_euclid(i64::from(s)) as i32)),
            leaf: (self.cell / leaf).round_ties_even() as u32,
        }
    }
}
