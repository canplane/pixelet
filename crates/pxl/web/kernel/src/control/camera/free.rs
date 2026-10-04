// pxl-kernel/control/camera/free.rs

//! The free camera turns about its eye, which is the map center. Its height
//! is no control: the target stands where the pan's walk stands it, and the
//! drawn eye eases to it, so a step comes as every other move does.

use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::{add3, V3};

use crate::control::camera::config::camera_cfg::free::EYE_HEIGHT_U;
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::DEFAULT_DIST;
use crate::control::camera::controller::{camera, eye_local, set_eye_local};
use crate::control::camera::controller::{damp_alpha, look_domain, spend_travel, units_per_px_at};
use crate::control::camera::store::camera_store;
use crate::control::navigation::support::target;
use crate::world::access::{Access, Detail, Ray};
use crate::world::frame::view::{view, view_compose};
use xpute_runtime::global::Global;

/// The first resident thing it meets, else the sea, else `fallback` along it.
pub fn look_point(eye: V3, forward: V3, fallback: f64) -> V3 {
    let frame = ivec::to_v3(view().origin);
    let ray = Ray {
        origin: add3(&eye, &frame),
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

/// Canonical, so a rebase leaves it where it is. None until first put.
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
    let eye = at;
    *to() = Some(eye);
    if snap {
        set_eye_local(&canonical_to_local(&eye, view().origin));
    }
}

/// Whether still moving.
pub(crate) fn turn(delta: f64) -> bool {
    let walked = spend_travel(delta).is_some_and(|share| {
        let Some(from) = eye_local() else { return false };
        let mut to = [from[0] + share[0], from[1] + share[1], from[2]];
        stand(&mut to);
        set_eye_local(&to);
        true
    });
    let vs = camera_store().intent;
    let cam = camera();
    let heading = wrap(vs.azimuth - cam.yaw, -core::f64::consts::PI, core::f64::consts::PI);
    let tilt = vs.polar - cam.pitch;
    (cam.pitch, cam.yaw) = look_domain(cam.pitch + tilt * damp_alpha(CAM_DAMP, delta), cam.yaw + heading * damp_alpha(ROT_DAMP, delta));
    view_compose();
    walked || heading.abs() > 1e-4 || tilt.abs() > 1e-4
}
