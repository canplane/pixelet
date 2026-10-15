// pxl-spec/procedural.rs

//! What the client makes and the bake never stores: scattered decor, and
//! where a tile's bridge piers stand (pxl-portable ways.rs). Pure functions
//! of the world and a seed, so every reader agrees.

use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::units;
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::tile::Tile;

/// A node's seed and the caves, one source with the GPU's (pxl-portable).
pub use pxl_portable::caves::{node_seed, Cave, CAVE_COARSEST};
use pxl_portable::ways::{Stations, INTERVAL_M};
use pxl_portable::LEAF_CELL;

// ---- scatter: portable's, for the CPU's readers of one cell ----

pub use pxl_portable::scatter::{before, each_candidate, meet, pose, scatter_at, side_of, Candidate, ALIGN_NEIGHBOR_RADIUS};

// ---- piers ----

/// A tile's cells in the world, so a station is the world's and not a tile's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub corner: [f64; 2], // south-west, in u
    pub cell: f64,        // a cell's side, in u
}

impl Frame {
    pub fn of(tile: Tile) -> Frame {
        let (x0, y0, side) = tile.rect();
        Frame {
            corner: [x0, y0],
            cell: side / f64::from(TILE_CELLS),
        }
    }

    /// Where its piers may stand, in its cells (pxl-portable ways.rs
    /// `Stations`): each beam's lattice of stations, at its spacing in
    /// ground metres where the tile lies, and where its corner falls in it;
    /// and the leaf cells its cell spans.
    pub fn stations(self) -> Stations {
        let corner = self.corner.map(|v| v / self.cell);
        let step = INTERVAL_M.map(|m| f64::from(m) * u_per_m(self.corner[1]) / self.cell);
        Stations {
            step: step.map(|s| s as f32),
            phase: step.map(|s| corner.map(|c| c.rem_euclid(s) as f32)),
            leaf: (self.cell / units(LEAF_CELL)) as f32,
        }
    }
}

#[cfg(test)]
#[path = "procedural.test.rs"]
mod test;
