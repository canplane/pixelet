// pxl-kernel/control/camera/free.rs

//! The free camera. It turns the camera (controller.rs `Camera`) about its
//! eye, which is the map center, and walks it over the ground (`pan`), at one
//! speed wherever it stands. The eye's height is no control: it is the
//! ground's where it stands, and `EYE_HEIGHT_U` over it (`stand`).
//!
//! As the orbit's, its target and what is drawn are kept apart: the controls
//! write where the eye is to stand and the look in the camera store's
//! intent, and `turn` brings the drawn eye and look to them by the one law
//! (approach.rs). The ground sets the target's height as the drag sets its X
//! and Y, so a step in the ground comes as every other move does.

use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::v3::V3;

use crate::control::camera::config::camera_cfg::free::EYE_HEIGHT_U;
use crate::control::camera::approach::{approach_angle, approach_scalar, approach_vec};
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::DEFAULT_DIST;
use crate::control::camera::controller::{camera, eye_local, set_eye_local};
use crate::control::camera::controller::{look_domain, units_per_px_at};
use crate::global::Global;
use crate::control::camera::store::camera_store;
use crate::world::access::{Access, Detail, Ray, FINEST};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::{canonical_to_local, local_height, local_world_to_canonical};
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

/// Where the controls have the eye stand, canonical, so a rebase leaves it
/// where it is: what the drawn eye comes to. None until the camera is first
/// put anywhere.
static TO: Global<Option<Pvf3>> = Global::new();

fn to() -> &'static mut Option<Pvf3> {
    TO.get(|| None)
}

/// How close the drawn eye comes, in units, and the look, in radians,
/// before each takes its target whole: sub-pixel at any distance.
const EYE_EPS: f64 = 0.001;
const LOOK_EPS: f64 = 0.0001;

/// A drag walks the eye's target over the ground: `dx`, `dy` in pixels. One
/// speed wherever it stands and whichever way it goes, and the orbit's at
/// the distance it starts at (`DEFAULT_DIST`): a drag carries the ground as
/// far under either.
pub fn pan(dx: f64, dy: f64) {
    let Some(from) = to().or_else(|| eye_local().map(|e| canonical(&e))) else { return };
    let units_per_px = units_per_px_at(DEFAULT_DIST);
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = camera().yaw.sin_cos();
    let at = canonical_to_local(&from, view().origin);
    let mut next = [at[0] + lx * c + ly * s, at[1] - lx * s + ly * c, at[2]];
    stand(&mut next);
    *to() = Some(canonical(&next));
    request_world_tick();
}

/// The eye's target put standing at `at`, `EYE_HEIGHT_U` over the ground
/// there: where a place said outright puts it, and with `snap` the drawn
/// eye there at once.
pub(crate) fn put(at: Pvf3, snap: bool) {
    let mut eye = canonical_to_local(&at, view().origin);
    stand(&mut eye);
    *to() = Some(canonical(&eye));
    if snap {
        set_eye_local(&eye);
    }
}

fn canonical(local: &V3) -> Pvf3 {
    let mut out = Pvf3::default();
    local_world_to_canonical(&mut out, local, view().origin);
    out
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

/// The free camera's turn over `delta` seconds: its eye and its look each
/// brought toward their targets. Whether it is still moving.
pub(crate) fn turn(delta: f64) -> bool {
    let mut moving = false;
    if let Some(eye) = eye_local() {
        let target = to().map_or(eye, |at| canonical_to_local(&at, view().origin));
        let (next, m) = approach_vec(eye, target, POS_DAMP, delta, EYE_EPS);
        if next != eye {
            set_eye_local(&next);
        }
        moving |= m;
    }
    let vs = camera_store().intent;
    let cam = camera();
    let (pitch, m) = approach_scalar(cam.pitch, vs.polar, CAM_DAMP, delta, LOOK_EPS);
    moving |= m;
    let (yaw, m) = approach_angle(cam.yaw, vs.azimuth, ROT_DAMP, delta, LOOK_EPS);
    moving |= m;
    (cam.pitch, cam.yaw) = look_domain(pitch, yaw);
    view_compose();
    moving
}
