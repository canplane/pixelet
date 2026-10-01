// pxl-kernel/layer/object/size.rs

//! What a placed object may be sized to.
//!
//! The shell names a size by its tier and never by a length: which tiers
//! exist and what each measures is this ladder's, sent to it in the
//! snapshot, so a change of the cell rescales every tier with nothing on the
//! other side to follow.

use pxl_spec::bake::constant::{CELL, OBJECT_TIERS};

/// Grid-aligned steps down from one terrain cell, coarsest first: tier `i`
/// is the cell `i` levels down. The largest is the terrain's own cell, so an
/// object that size lines up 1:1 with a cell, and the finer tiers are what
/// the object paint writes as sub-cell voxels within the volume's dense
/// field (pxl-spec bake/constant.rs's OBJECT_LAYOUT, one level a tier).
pub const OBJECT_SIZES: [f64; OBJECT_TIERS] = {
    let mut sizes = [0.0; OBJECT_TIERS];
    let mut i = 0;
    while i < OBJECT_TIERS {
        sizes[i] = CELL.down(i as u32).side();
        i += 1;
    }
    sizes
};

/// The tier a fresh session places at: the coarsest, one full cell.
pub const DEFAULT_OBJECT_SIZE: f64 = OBJECT_SIZES[0];

/// Every decor kind places at exactly one terrain cell — unlike a color
/// cube, decor has no mode_bar size flyout of its own (yet) to vary this, and
/// a scattered tree already assumes its own candidates are one full cell
/// wide. Deliberately the same value as OBJECT_SIZES[0], not a re-export of
/// it — object_size is a live, user-changed selection for the object tool
/// specifically; this is decor's own fixed constant and must keep working
/// even if a future size flyout moves object_size elsewhere.
pub const DECOR_SIZE: f64 = OBJECT_SIZES[0];

/// The size tier `tier` places at; the default for a tier the ladder does
/// not have.
pub fn object_size_of(tier: u32) -> f64 {
    OBJECT_SIZES.get(tier as usize).copied().unwrap_or(DEFAULT_OBJECT_SIZE)
}

/// The tier a thing `len` long at its longest is filled at: the coarsest
/// that is no longer than it, so one a cell long or more fills whole cells
/// and a smaller one the sub-cells of its own size; the finest below that.
pub fn tier_of(len: f64) -> f64 {
    OBJECT_SIZES.into_iter().find(|&side| side <= len).unwrap_or(OBJECT_SIZES[OBJECT_TIERS - 1])
}
