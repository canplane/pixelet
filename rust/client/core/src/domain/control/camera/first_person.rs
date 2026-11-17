// pxl-core/domain/control/camera/first_person.rs

//! The first-person camera turns about its eye, which is also its pivot. No
//! control sets its height. The pivot's `to` (state.rs `CameraState`) stands
//! wherever the pan's walk leaves it, and the drawn eye eases toward it, so a
//! step up or down eases like every other move.

use crate::domain::control::camera::state::{camera_state, ease_pivot, resolved_to};
use crate::domain::control::camera::{
    camera, ease_facing, eases_toward, eye_world, facing_domain, set_eye_world,
};
use crate::domain::control::param::CAM_DAMP;

/// Puts the eye at the pivot's `to` at once.
pub(crate) fn snap() {
    set_eye_world(&resolved_to(eye_world().unwrap_or_default()));
}

/// Eases the drawn eye and facing over `delta` seconds and returns whether
/// they are still moving.
pub(crate) fn step(delta: f64) -> bool {
    let Some(eye) = eye_world() else {
        return false;
    };
    let vs = camera_state().intent;
    let cam = camera();
    let to = resolved_to(eye);
    let (polar, azimuth) = facing_domain(vs.polar, vs.azimuth);
    let moving = eases_toward(cam, to, polar, azimuth);
    let next = if moving {
        ease_facing(cam, polar, CAM_DAMP, azimuth, delta);
        ease_pivot(eye, to, delta)
    } else {
        (cam.polar, cam.azimuth) = (polar, azimuth);
        to
    };
    if next != eye {
        set_eye_world(&next);
    }
    moving
}
