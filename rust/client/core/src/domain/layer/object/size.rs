// pxl-core/domain/layer/object/size.rs

//! The sizes a placed object may take. The UI names a size by its scale
//! step, never by a length, so changing the cell size rescales every object.

use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_SCALES};
use pxl_spec::space::coord::zoom::Zoom;
use pxl_spec::space::layer::lattice::GROUND;

/// The zoom of a marker's cube: one ground leaf cell, whatever size the
/// object tool is set to.
pub const MARKER_ZOOM: Zoom = GROUND.leaf_cell_zoom();

/// The zoom of a cube at scale step `scale_steps`, read as a decor's zoom is
/// (`Decor::cell_zoom`). Step 0 is the coarsest object size,
/// `OBJECT_COARSEST`, and each step down halves the cube's side. A step
/// outside the object scales gives the coarsest zoom.
pub fn object_zoom_of(scale_steps: i32) -> Zoom {
    if (1 - OBJECT_SCALES as i32..=0).contains(&scale_steps) {
        (OBJECT_COARSEST as i32 - scale_steps) as Zoom
    } else {
        OBJECT_COARSEST
    }
}
