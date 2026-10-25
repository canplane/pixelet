// pxl-kernel/domain/layer/object/size.rs

//! What a placed object may be sized to. The shell names a size by tier,
//! never by length, so changing the cell rescales every tier.

use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_TIERS};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;

/// A marker's place: one ground cell, not tied to the object tool's live size.
pub const MARKER_ZOOM: Zoom = GROUND.cell();

/// Tier `i` is the cell `i` steps finer, coarsest first; a tier the ladder
/// does not have is the coarsest.
pub fn object_zoom_of(tier: u32) -> Zoom {
    match (tier as usize) < OBJECT_TIERS {
        true => OBJECT_COARSEST + tier as Zoom,
        false => OBJECT_COARSEST,
    }
}
