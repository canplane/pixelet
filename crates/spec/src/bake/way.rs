// pxl-spec/bake/way.rs

//! How far a way covers the cells along its line, on the ground and on a deck
//! alike. A way covers a cell whose center is within half its width, and that
//! half is at least half a cell's diagonal so a diagonal way stays side-connected.
//! An approach's look is laid into a leaf's cells (`lay`), so every zoom
//! folds it as the ground's; its embankment or cutting stays a line.

use core::f64::consts::FRAC_1_SQRT_2;

use fnv::FnvHashMap;
use xpute_kit::math::scalar::round;

use super::separation::{Form, Separation};
use super::tile::terrain::GroundId;
use crate::block::cube::{sentinel, Ground};
use crate::procedural::Frame;
use crate::space::layer::lattice::TILE_CELLS;

/// Half a way's width, in whole cells.
pub fn half_cells(width: f64) -> i32 {
    round(width / 2.0) as i32
}

/// How far from its line a way of `width` cells covers a cell's center.
pub fn reach(width: f64) -> f64 {
    (width * 0.5).max(FRAC_1_SQRT_2)
}

pub fn covers(width: f64, distance: f64) -> bool {
    distance <= reach(width)
}

/// The looks of a leaf at `frame` an approach's line crosses, laid: each
/// such cell the way's kind, the highest's where two cross one. Bedrock and
/// kinds that do not ground are left as they are, as the ground's face takes
/// no way there; an embankment's or a cutting's height stays its line's.
pub fn lay(sep: &Separation, frame: Frame, looks: &mut [GroundId]) {
    let n = i64::from(TILE_CELLS);
    // per cell: the highest top over it, and its kind
    let mut laid: FnvHashMap<usize, (f64, u8)> = FnvHashMap::default();
    sep.crossing_grid(Some(frame), [0.0, 0.0], 1.0, [n, n], |x, y, _, o| {
        let col = (y * n + x) as usize;
        if o.form != Form::Approach || looks[col] == sentinel::unknown || !Ground::grounds(u32::from(o.kind)) {
            return;
        }
        let e = laid.entry(col).or_insert((f64::MIN, o.kind));
        if o.top > e.0 {
            *e = (o.top, o.kind);
        }
    });
    for (col, (_, kind)) in laid {
        looks[col] = kind;
    }
}
