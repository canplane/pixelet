// pxl-spec/bake/ground.rs

//! How high a ground cell stands. A cell is its look; a void one is where the
//! terrain gives no surface, past the source's coverage.

use crate::cell::cube::cube_types::Ground;

use super::format::GroundCell;

/// A raise or a sink that sets a look's cells apart from the ground around
/// them, up positive: the catalog's `shift`.
pub fn cell_shift(cell: GroundCell) -> i16 {
    Ground::of(cell).map_or(0, Ground::shift)
}

/// In cells.
pub const SHIFT_RANGE: [i16; 2] = {
    let (mut lo, mut hi) = (0, 0);
    let mut i = 0;
    while i < Ground::ALL.len() {
        let r = Ground::ALL[i].shift();
        if r < lo {
            lo = r;
        }
        if r > hi {
            hi = r;
        }
        i += 1;
    }
    [lo, hi]
};
