// pxl-kernel/domain/control/camera/first_person.rs

//! The first-person camera turns about its eye, which is the pivot. Its height
//! is no control: its `to` stands where the pan's walk stands it, and the
//! drawn eye eases to it, so a step comes as every other move does.

use pxl_spec::space::coord::q3::Q3;

use crate::domain::approach::{approach_angle, approach_vec};
use crate::domain::control::camera::controller::facing_domain;
use crate::domain::control::camera::controller::{camera, eye_world, pivot, set_eye_world};
use crate::domain::control::camera::state::camera_state;
use crate::domain::control::navigation::support::resolve_to;
use crate::domain::control::param::{
    CAM_DAMP, FIRST_PERSON_EYE_EPS, FIRST_PERSON_FACING_EPS, ROT_DAMP,
};
use crate::domain::param::POS_DAMP;
use xpute_guest::global::Global;

/// Canonical, so it holds still as the eye goes round the world. None until first put.
static TO: Global<Option<Q3>> = Global::new();

fn to() -> &'static mut Option<Q3> {
    TO.get(|| None)
}

/// First person, where the eye goes to is where the pivot goes to.
pub(crate) fn pivot_to() -> Option<Q3> {
    to().or_else(pivot)
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
        let (next, m) = approach_vec(eye, to, POS_DAMP, delta, FIRST_PERSON_EYE_EPS);
        if next != eye {
            set_eye_world(&next);
        }
        moving |= m;
    }
    let vs = camera_state().intent;
    let cam = camera();
    let ([polar], m) = approach_vec(
        [cam.polar],
        [vs.polar],
        CAM_DAMP,
        delta,
        FIRST_PERSON_FACING_EPS,
    );
    moving |= m;
    let (azimuth, m) = approach_angle(
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
