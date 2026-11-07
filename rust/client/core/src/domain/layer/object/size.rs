// pxl-core/domain/layer/object/size.rs

//! What a placed object may be sized to. The UI names a size by its scale,
//! never by length, so changing the cell rescales every one.

use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_SCALES};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;

/// A marker's place: one ground cell, not tied to the object tool's live size.
pub const MARKER_ZOOM: Zoom = GROUND.cell();

/// The zoom a cube of `scale_steps` is at, as a decor's (`Decor::cell_zoom`): a
/// unit's, that many coarser, so each step down halves it. One past the object
/// scales is the coarsest.
pub fn object_zoom_of(scale_steps: i32) -> Zoom {
    if (1 - OBJECT_SCALES as i32..=0).contains(&scale_steps) {
        (OBJECT_COARSEST as i32 - scale_steps) as Zoom
    } else {
        OBJECT_COARSEST
    }
}
