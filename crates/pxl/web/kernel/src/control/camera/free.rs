// pxl-kernel/control/camera/free.rs

//! The free camera. It turns the camera (controller.rs `Camera`) about its
//! eye, which is the map center, and walks it over the ground (`pan`), at one
//! speed wherever it stands, eased as the orbit's drag is. The eye's height
//! is no control: it is the ground's where it stands, and `EYE_HEIGHT_U` over
//! it (`stand`). It keeps no state of its own.

use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::wrap;

use crate::control::camera::config::camera_cfg::free::EYE_HEIGHT_U;
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::DEFAULT_DIST;
use crate::control::camera::controller::{camera, eye_local, set_eye_local};
use crate::control::camera::controller::{damp_alpha, look_domain, spend_travel, units_per_px_at};
use crate::control::camera::store::camera_store;
use crate::world::access::{Access, Detail, Ray, FINEST};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::local_height;
use crate::world::frame::view::{view, view_compose};

/// What a look from `eye` along `forward` falls on, in the root frame: the
/// first thing it meets that is resident, else the sea, else `fallback`
/// along it.
pub fn look_point(eye: V3, forward: V3, fallback: f64) -> V3 {
    let o = view().origin;
    let frame = [ivec::to_real(ivec::X(o)), ivec::to_real(ivec::Y(o)), ivec::to_real(ivec::Z(o))];
    let ray = Ray {
        origin: std::array::from_fn(|k| eye[k] + frame[k]),
        direction: forward,
        // As far as the view reaches: past it nothing is drawn to look at.
        reach: view().far,
    };
    // Far along it the ground is read as coarse as it is drawn there.
    let t = match Access::now().first_hit(&ray, Detail::Distance(Tau::FINEST)).value().flatten() {
        Some(hit) => hit.t,
        // The sea is z = 0 in the world's units.
        None if forward[2] < -1e-9 && ray.origin[2] > 0.0 => -ray.origin[2] / forward[2],
        None => fallback,
    };
    std::array::from_fn(|k| eye[k] + forward[k] * t)
}

/// A drag walks the eye over the ground: `dx`, `dy` in pixels, over the
/// turns to come (controller.rs `spend_travel`). One speed wherever it stands
/// and whichever way it goes, and the orbit's at the distance it starts at
/// (`DEFAULT_DIST`): a drag carries the ground as far under either.
pub fn pan(dx: f64, dy: f64) {
    if eye_local().is_none() {
        return;
    }
    let units_per_px = units_per_px_at(DEFAULT_DIST);
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = camera().yaw.sin_cos();
    let travel = &mut camera_store().travel;
    travel[0] += lx * c + ly * s;
    travel[1] += -lx * s + ly * c;
    request_world_tick();
}

/// The eye stood where it was put: a place put outright is where it stands,
/// `EYE_HEIGHT_U` over the ground there.
pub fn hold() {
    let Some(at) = eye_local() else { return };
    let mut to = at;
    stand(&mut to);
    if to != at {
        set_eye_local(&to);
    }
}

/// Where an eye over `eye`'s X and Y stands: within the lattice's rows
/// north and south, and `EYE_HEIGHT_U` over the ground there. Its Z is the ground's and nothing the hand moves; where the ground
/// there is not known yet, it keeps the Z it has, and the ground arriving
/// later moves nothing — only a step or a place put reads the ground again.
fn stand(eye: &mut V3) {
    let o = view().origin;
    let base = [ivec::to_real(ivec::X(o)), ivec::to_real(ivec::Y(o))];
    let y = (eye[1] + base[1]).clamp(REACH.min[1], REACH.max[1] - 1.0);
    eye[1] = y - base[1];
    let mut under = Pvf3::default();
    projection::project(&mut under, eye[0] + base[0], y, 0.0);
    if let Some(ground) = Access::now().at(&under, FINEST).value() {
        eye[2] = local_height(ground.top, o) + EYE_HEIGHT_U;
    }
}

/// The free camera's turn over `delta` seconds: its look eased toward the
/// controls' heading and tilt, and its eye walked by this turn's share of the
/// hand's move, standing on the ground it walks to. Whether it is still
/// moving.
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
