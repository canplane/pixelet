// pxl-kernel/layer/object/size.rs

//! What a placed object may be sized to. The shell names a size by tier,
//! never by length, so changing the cell rescales every tier.

use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_TIERS};
use pxl_spec::space::coord::zoom::{units, Zoom};
use pxl_spec::space::layer::lattice::GROUND;

/// Tier `i` is the cell `i` steps finer, coarsest first.
pub const OBJECT_SIZES: [f64; OBJECT_TIERS] = {
    let mut sizes = [0.0; OBJECT_TIERS];
    let mut i = 0;
    while i < OBJECT_TIERS {
        sizes[i] = units(OBJECT_COARSEST + i as Zoom);
        i += 1;
    }
    sizes
};

pub const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

/// One ground cell, as the scatter assumes. Its own constant, not tied to
/// the object tool's live size.
pub const DECOR_SIZE: f64 = units(GROUND.cell());

/// The default for a tier the ladder does not have.
pub fn object_size_of(tier: u32) -> f64 {
    OBJECT_SIZES.get(tier as usize).copied().unwrap_or(DEFAULT_OBJECT_SIZE)
}
