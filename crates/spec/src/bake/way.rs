// pxl-spec/bake/way.rs

//! How far a way covers the cells along its line, on the ground and on a deck
//! alike. A way covers a cell whose center is within half its width, and that
//! half is at least half a cell's diagonal so a diagonal way stays side-connected.
//! An approach's look is laid into a leaf's cells (`lay`), so every zoom
//! folds it as the ground's; its embankment or cutting stays a line.

use fnv::FnvHashMap;

use super::separation::{Form, Separation};
use super::tile::terrain::GroundId;
use crate::block::cube::{sentinel, Ground};
use crate::procedural::Frame;
use crate::space::layer::lattice::TILE_CELLS;

/// A way's half width in whole cells, and how far from its line it covers a
/// cell's center (pxl-portable ways.rs).
pub use pxl_portable::ways::{covers, half_cells, reach};

/// The looks of a leaf at `frame` an approach's line crosses, laid: each
/// such cell the way's kind, the highest's where two cross one. Bedrock and
/// kinds that do not ground are left as they are, as the ground's face takes
/// no way there; an embankment's or a cutting's height stays its line's.
pub fn lay(sep: &Separation, frame: Frame, looks: &mut [GroundId]) {
    let n = i64::from(TILE_CELLS);
    // per cell: the highest top over it, and its kind
    let mut laid: FnvHashMap<usize, (f32, u8)> = FnvHashMap::default();
    sep.crossing_grid(frame, [0.0, 0.0], 1.0, [n, n], |x, y, _, o| {
        let col = (y * n + x) as usize;
        if o.form != u32::from(Form::Approach.byte()) || looks[col] == sentinel::unknown || !Ground::grounds(o.kind) {
            return;
        }
        let e = laid.entry(col).or_insert((f32::MIN, o.kind as u8));
        if o.top > e.0 {
            *e = (o.top, o.kind as u8);
        }
    });
    for (col, (_, kind)) in laid {
        looks[col] = kind;
    }
}
