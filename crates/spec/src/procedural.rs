// pxl-spec/procedural.rs

//! What the client makes and the bake never stores: scattered decor, and
//! where a tile's bridge piers stand (pxl-portable ways.rs). Pure functions
//! of the world and a seed, so every reader agrees.

use std::collections::TryReserveError;

use pxl_portable::decor::Decor;

use crate::space::coord::adapter::epsg3857::u_per_m;
use crate::space::coord::zoom::{units, Zoom};
use crate::space::layer::lattice::TILE_CELLS;
use crate::space::layer::tile::Tile;

/// A node's seed and the caves, one source with the GPU's (pxl-portable).
pub use pxl_portable::caves::{node_seed, Cave, CAVE_COARSEST};
use pxl_portable::ways::{Stations, INTERVAL_M};
use pxl_portable::LEAF_CELL;

// ---- scatter: what stands in a region, by portable's ----

pub use pxl_portable::scatter::{before, each_candidate, meet, pose, scatter_at, side_of, Candidate, ALIGN_NEIGHBOR_RADIUS};

/// What stands at `z` in `region` (leaf cells, `[min, max)`), by its cell, as
/// `scatter_at` keeps each. Each candidate claims the square `side_of` gives
/// it, from its cell up, which holds its box however it turns; of two whose
/// squares meet, the one `before` the other stands.
pub fn scatter(decor: &Decor, zoom: Zoom, region: [[i32; 2]; 2], out: &mut Vec<Candidate>) -> Result<(), TryReserveError> {
    out.clear();
    let reach = decor.widest() as i32 - 1;
    let around = [region[0].map(|v| v - reach), region[1].map(|v| v + reach)];
    let mut grown = Ok(());
    each_candidate(decor, zoom, around, |c| {
        if grown.is_ok() {
            grown = out.try_reserve(1).map(|()| out.push(c));
        }
    });
    grown?;
    out.sort_unstable_by(|a, b| a.cell.cmp(&b.cell).then(a.born.cmp(&b.born)).then(a.seed.cmp(&b.seed)));
    // Survivors go past the end, then the whole is let go of.
    let n = out.len();
    for i in 0..n {
        let c = out[i];
        if !(0..2).all(|k| (region[0][k]..region[1][k]).contains(&c.cell[k])) {
            continue;
        }
        let side = side_of(decor, &c);
        // Only a candidate within `reach` before it on x, or its own side past, can meet it.
        let from = out[..n].partition_point(|d| d.cell[0] < c.cell[0] - reach);
        let beaten = out[from..n]
            .iter()
            .take_while(|d| d.cell[0] < c.cell[0] + side)
            .any(|d| before(d, &c) && meet(d, side_of(decor, d), &c, side));
        if !beaten {
            out.try_reserve(1)?;
            out.push(c);
        }
    }
    out.drain(..n);
    Ok(())
}

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
