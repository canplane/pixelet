// pxl-kernel/control/camera/free.rs

//! The free camera turns about its eye, which is the map center. Its height
//! is no control: the target stands where the pan's walk stands it, and the
//! drawn eye eases to it, so a step comes as every other move does.

use pxl_spec::space::camera::grid::Tau;
use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::V3;

use crate::control::camera::approach::{approach_angle, approach_scalar, approach_vec};
use crate::param::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::control::camera::controller::look_domain;
use crate::control::camera::controller::{camera, eye_world, map_center, set_eye_world};
use crate::control::camera::store::camera_store;
use crate::control::navigation::support::target;
use crate::world::query::{Access, Detail, Ray};
use crate::world::view::frame::pose::{view, view_compose};
use xpute_runtime::global::Global;

/// The first resident thing it meets, else the sea, else `fallback` along it.
pub fn look_point(eye: V3, forward: V3, fallback: f64) -> V3 {
    let ray = Ray {
        origin: eye,
        direction: forward,
        reach: view().far,
    };
    let t = match Access::now().first_hit(&ray, Detail::Distance(Tau::FINEST)).value().flatten() {
        Some(hit) => hit.t,
        // The sea is z = 0.
        None if forward[2] < -1e-9 && ray.origin[2] > 0.0 => -ray.origin[2] / forward[2],
        None => fallback,
    };
    std::array::from_fn(|k| eye[k] + forward[k] * t)
}

/// Canonical, so it holds still as the eye goes round the world. None until first put.
static TO: Global<Option<Pvf3>> = Global::new();

fn to() -> &'static mut Option<Pvf3> {
    TO.get(|| None)
}

// Sub-pixel at any distance.
const EYE_EPS: f64 = 0.001;
const LOOK_EPS: f64 = 0.0001;

/// First person, the eye's target is the map center.
pub(crate) fn target_center() -> Option<Pvf3> {
    to().or_else(map_center)
}

pub(crate) fn put(at: Pvf3, snap: bool) {
    *to() = Some(at);
    if snap {
        set_eye_world(&target(to(), eye_world().unwrap_or_default()));
    }
}

/// Whether still moving.
pub(crate) fn turn(delta: f64) -> bool {
    let mut moving = false;
    if let Some(eye) = eye_world() {
        let to = target(to(), eye);
        let (next, m) = approach_vec(eye, to, POS_DAMP, delta, EYE_EPS);
        if next != eye {
            set_eye_world(&next);
        }
        moving |= m;
    }
    let vs = camera_store().intent;
    let cam = camera();
    let (polar, m) = approach_scalar(cam.polar, vs.polar, CAM_DAMP, delta, LOOK_EPS);
    moving |= m;
    let (azimuth, m) = approach_angle(cam.azimuth, vs.azimuth, ROT_DAMP, delta, LOOK_EPS);
    moving |= m;
    (cam.polar, cam.azimuth) = look_domain(polar, azimuth);
    view_compose();
    moving
}
