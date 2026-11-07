// pxl-core/domain/control/camera/first_person.rs

//! The first-person camera turns about its eye, which is the pivot. Its height
//! is no control: its `to` stands where the pan's walk stands it, and the
//! drawn eye eases to it, so a step comes as every other move does.

use pxl_spec::space::coord::q3::Q3;

use crate::domain::control::camera::state::camera_state;
use crate::domain::control::camera::{camera, eye_world, facing_domain, set_eye_world};
use crate::domain::control::param::{
    CAM_DAMP, FIRST_PERSON_EYE_EPS, FIRST_PERSON_FACING_EPS, ROT_DAMP,
};
use crate::domain::control::support::resolve_to;
use crate::domain::param::POS_DAMP;
use crate::domain::smooth::{smooth_angle, smooth_vec};
use crate::domain::view::frame::coord::world_to_canonical;
use xpute_core::global::Global;

/// Canonical, so it holds still as the eye goes round the world. None until first put.
static TO: Global<Option<Q3>> = Global::new();

fn to() -> &'static mut Option<Q3> {
    TO.get(|| None)
}

/// First person, where the eye goes to is where the pivot goes to.
pub(crate) fn pivot_to() -> Option<Q3> {
    to().or_else(|| eye_world().map(|e| world_to_canonical(&e)))
}

pub(crate) fn put(at: Q3, snap: bool) {
    *to() = Some(at);
    if snap {
        set_eye_world(&resolve_to(to(), eye_world().unwrap_or_default()));
    }
}

/// Whether still moving.
pub(crate) fn turn(delta: f64) -> bool {
    let mut moving = false;
    if let Some(eye) = eye_world() {
        let to = resolve_to(to(), eye);
        let (next, m) = smooth_vec(eye, to, POS_DAMP, delta, FIRST_PERSON_EYE_EPS);
        if next != eye {
            set_eye_world(&next);
        }
        moving |= m;
    }
    let vs = camera_state().intent;
    let cam = camera();
    let ([polar], m) = smooth_vec(
        [cam.polar],
        [vs.polar],
        CAM_DAMP,
        delta,
        FIRST_PERSON_FACING_EPS,
    );
    moving |= m;
    let (azimuth, m) = smooth_angle(
        cam.azimuth,
        vs.azimuth,
        ROT_DAMP,
        delta,
        FIRST_PERSON_FACING_EPS,
    );
    moving |= m;
    (cam.polar, cam.azimuth) = facing_domain(polar, azimuth);
    moving
}
