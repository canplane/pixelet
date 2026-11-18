// pxl-core/domain/control/camera/first_person.rs

//! The first-person camera turns about its eye, which stands over its pivot
//! (`eye_off`): the pivot is where a body stands over its floor, and the eye
//! half a cell under the top of the body's box (query/support.rs `over`, `eye_over`). No
//! control sets the height. The pivot's `to` (state.rs `CameraState`) stands
//! wherever the pan's walk leaves it, and the drawn eye eases toward it, so a
//! step up or down eases like every other move.

use pxl_spec::space::coord::v3::{self, V3};

use crate::domain::control::camera::state::{camera_state, ease_pivot, resolved_to};
use crate::domain::control::camera::{
    camera, ease_facing, eases_toward, eye_world, facing_domain, set_eye_world,
};
use crate::domain::control::param::CAM_DAMP;
use crate::domain::query::{eye_over, over};

/// The eye's place over the pivot, from where a body stands to where its eye
/// does.
fn eye_off() -> V3 {
    [0.0, 0.0, eye_over(0.0) - over(0.0)]
}

/// The pivot under the eye, None before the camera has an eye.
pub(crate) fn pivot_world() -> Option<V3> {
    eye_world().map(|eye| v3::sub(&eye, &eye_off()))
}

/// Puts the eye over the pivot's `to` at once.
pub(crate) fn snap() {
    let at = resolved_to(pivot_world().unwrap_or_default());
    set_eye_world(&v3::add(&at, &eye_off()));
}

/// Eases the drawn eye and facing over `delta` seconds and returns whether
/// they are still moving.
pub(crate) fn step(delta: f64) -> bool {
    let Some(drawn_pivot) = pivot_world() else {
        return false;
    };
    let vs = camera_state().intent;
    let cam = camera();
    let to = resolved_to(drawn_pivot);
    let (polar, azimuth) = facing_domain(vs.polar, vs.azimuth);
    let moving = eases_toward(cam, v3::add(&to, &eye_off()), polar, azimuth);
    let at = if moving {
        ease_facing(cam, polar, CAM_DAMP, azimuth, delta);
        ease_pivot(drawn_pivot, to, delta)
    } else {
        (cam.polar, cam.azimuth) = (polar, azimuth);
        to
    };
    if at != drawn_pivot {
        set_eye_world(&v3::add(&at, &eye_off()));
    }
    moving
}
