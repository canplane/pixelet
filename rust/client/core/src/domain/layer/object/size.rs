// pxl-core/domain/layer/object/size.rs

//! The sizes a placed object may take. The UI names a size by its scale
//! step, never by a length, so changing the cell size rescales every object.

use pxl_spec::bake::constant::{OBJECT_COARSEST, OBJECT_SCALE_CNT};
use pxl_spec::space::coord::zoom::Zoom;

/// The zoom of a cube at scale step `scale_step`. Step 0 is the coarsest
/// object size, `OBJECT_COARSEST`, and each step down halves the cube's side.
/// A step outside the object scales gives the coarsest zoom.
pub(crate) fn object_zoom_of(scale_step: i32) -> Zoom {
    if (1 - OBJECT_SCALE_CNT as i32..=0).contains(&scale_step) {
        (OBJECT_COARSEST as i32 - scale_step) as Zoom
    } else {
        OBJECT_COARSEST
    }
}
