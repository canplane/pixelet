// pxl-core/domain/control/camera/first_person.rs

//! The first-person camera turns about its eye, which is also its pivot. No
//! control sets its height. The pivot's `to` (state.rs `CameraState`) stands
//! wherever the pan's walk leaves it, and the drawn eye eases toward it, so a
//! step up or down eases like every other move.

use crate::domain::control::camera::state::{camera_state, ease_pivot, resolved_to};
use crate::domain::control::camera::{camera, ease_facing, eye_world, set_eye_world};
use crate::domain::control::param::CAM_DAMP;

/// Puts the eye at the pivot's `to` at once.
pub(crate) fn snap() {
    set_eye_world(&resolved_to(eye_world().unwrap_or_default()));
}

/// Eases the drawn eye and facing over `delta` seconds and returns whether
/// they are still moving.
pub(crate) fn turn(delta: f64) -> bool {
    let mut moving = false;
    if let Some(eye) = eye_world() {
        let (next, m) = ease_pivot(eye, delta);
        if next != eye {
            set_eye_world(&next);
        }
        moving |= m;
    }
    let vs = camera_state().intent;
    moving | ease_facing(camera(), vs.polar, CAM_DAMP, vs.azimuth, delta)
}
