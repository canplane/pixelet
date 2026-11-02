// pxl-spec/procedural.rs

//! What the client makes and the bake never stores: scattered decor, and
//! where a tile's bridge piers stand (pxl-engine way.rs). Pure functions
//! of the world and a seed, so every reader agrees.

use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::units;
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::tile::{Tile, LEAF_ZOOM};

use pxl_engine::way::{Stations, INTERVAL_M};
use pxl_engine::LEAF_CELL;

// ---- scatter: pxl-engine's, for the CPU's readers of one cell ----

pub use pxl_content::scatter::{before, meet, scatter_at, side_of, Candidate};

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

    /// Leaf cells a ground meter over the tile, of height as of breadth: the
    /// scale where its south-west corner lies, one value a tile
    /// (pxl-engine `cells_of_m`). A leaf tile's is the scale the bake
    /// makes the ground's heights by, and so a height's meters
    /// (`height_u_per_m`).
    pub fn per_m(self) -> f32 {
        (u_per_m(self.corner[1]) / units(LEAF_CELL)) as f32
    }

    /// Where its piers may stand, in its cells (pxl-engine way.rs
    /// `Stations`): each beam's lattice of stations, at its spacing in
    /// ground meters where the tile lies, and where its corner falls in it;
    /// and the leaf cells its cell spans.
    pub(crate) fn stations(self) -> Stations {
        let corner = self.corner.map(|v| v / self.cell);
        let step = INTERVAL_M.map(|m| f64::from(m) * u_per_m(self.corner[1]) / self.cell);
        Stations {
            step: step.map(|s| s as f32),
            phase: step.map(|s| corner.map(|c| c.rem_euclid(s) as f32)),
            leaf: (self.cell / units(LEAF_CELL)) as f32,
        }
    }
}

/// The world's units a ground meter of height at `y_u`: the scale the bake
/// made the ground's heights by there, its leaf tile's (`Frame::per_m`, which
/// the terrain header keeps and the clip levels read). Every height shown or
/// read in meters is converted by it (pxl-spec 3857.rs `project`,
/// `unproject`).
pub fn height_u_per_m(y_u: f64) -> f64 {
    let row = libm::floor(y_u / units(LEAF_ZOOM)) as i32;
    f64::from(Frame::of(Tile::new(LEAF_ZOOM, [0, row])).per_m()) * units(LEAF_CELL)
}
