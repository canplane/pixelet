// pxl-kernel/layer/object/size.rs

//! What a placed object may be sized to. The shell names a size by tier,
//! never by length, so changing the cell rescales every tier.

use pxl_spec::bake::constant::{CELL, OBJECT_TIERS};

/// Tier `i` is the cell `i` levels down, coarsest first.
pub const OBJECT_SIZES: [f64; OBJECT_TIERS] = {
    let mut sizes = [0.0; OBJECT_TIERS];
    let mut i = 0;
    while i < OBJECT_TIERS {
        sizes[i] = CELL.down(i as u32).side();
        i += 1;
    }
    sizes
};

pub const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

/// One cell, as the scatter assumes. Its own constant, not tied to the
/// object tool's live size.
pub const DECOR_SIZE: f64 = OBJECT_SIZES[0];

/// The default for a tier the ladder does not have.
pub fn object_size_of(tier: u32) -> f64 {
    OBJECT_SIZES.get(tier as usize).copied().unwrap_or(DEFAULT_OBJECT_SIZE)
}

/// The tier a thing `len` long at its longest is filled at: the coarsest
/// that is no longer than it, so one a cell long or more fills whole cells
/// and a smaller one the sub-cells of its own size; the finest below that.
pub fn tier_of(len: f64) -> f64 {
    OBJECT_SIZES.into_iter().find(|&side| side <= len).unwrap_or(OBJECT_SIZES[OBJECT_TIERS - 1])
}
