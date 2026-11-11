// pxl-core/domain/control/camera/orbit.rs

//! The orbit camera. The eye moves on a sphere about the pivot, and the
//! sphere's radius is the zoom. The controls write what the camera is to
//! reach: the intent and the pivot's `to` (state.rs). `turn` eases the drawn
//! camera toward them (domain/smooth.rs). Everything that reads the world
//! reads the drawn camera.
//!
//! Every control is written through one function, `ask`. There the terrain
//! answers whether a state puts the eye in the ground (camera/collision.rs
//! `allowed_view`), and the obstacle setting decides what follows. With
//! `through`, the ground is ignored. With `stop`, a control goes as far as
//! it can. With `climb`, the drawn pitch rises instead (`climb_polar`).
//! Where the way of a control still hits something, a zoom in goes through
//! it (`dive`) and any other control stops (`Step`).

use pxl_spec::space::coord::v3::{self, V3};

use crate::domain::control::camera::collision::{allowed_view, climb_polar};
use crate::domain::control::camera::gesture::{spread, Anchor};
use crate::domain::control::camera::state::{
    camera_state, camera_state_mut, ease_pivot, resolved_to, Intent, IntentPatch,
};
use crate::domain::control::camera::{camera, ease_facing, facing_dir, set_eye_world};
use crate::domain::control::param;
use crate::domain::control::param::DOLLY_GAIN;
use crate::domain::control::param::{CAM_DAMP, CLIMB_RELAX_DAMP, CLIMB_RISE_DAMP, EASE_EPS_U};
use crate::domain::setting::store::{settings, OrbitObstacleMode};
use crate::domain::smooth::smooth_vec;
use xpute_core::global::Global;

pub struct OrbitState {
    pub dist: f64,
    pinch_from: Anchor,
}

static ORBIT: Global<OrbitState> = Global::new();

pub fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: param::DEFAULT_DIST,
        pinch_from: Anchor::new(param::DEFAULT_DIST, 1.0),
    })
}

pub fn orbit_eye_off(dist: f64, polar: f64, azimuth: f64) -> V3 {
    facing_dir(polar, azimuth).map(|v| -v * dist)
}

/// The point the orbit turns about, the drawn distance ahead of the eye.
/// None before the camera has an eye.
pub fn pivot_world() -> Option<V3> {
    let c = camera();
    let e = c.eye?;
    Some(v3::sub(
        &e,
        &orbit_eye_off(orbit().dist, c.polar, c.azimuth),
    ))
}

/// Writes a control into the intent and returns the intent taken. Terrain
/// holds the control back here, where it is written, so a control that
/// turns back moves the camera at once. Under `climb`, the ground is answered
/// by pitching the drawn facing in `turn`, because a pitch written into the
/// intent would stay after the ground no longer asks for it.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_state_mut();
    let asked = c.with_patch(partial);
    c.commit_intent(allowed_view(asked, c.intent));
    c.intent
}

pub fn zoom_settling() -> bool {
    (camera_state().intent.dist - orbit().dist).abs() > EASE_EPS_U
}

/// A wheel turn of `delta_y` CSS pixels.
pub fn wheel(delta_y: f64) {
    zoom_by(libm::exp(delta_y * DOLLY_GAIN));
}

/// A `factor` under one zooms closer.
pub fn zoom_by(factor: f64) {
    zoom_to(camera_state().intent.dist * factor);
}

pub fn pinch_begin(span: f64) {
    orbit().pinch_from = Anchor::new(camera_state().intent.dist, span);
}

/// The fingers are `span` apart. Spreading them zooms in.
pub fn pinch(span: f64) {
    let asked = orbit().pinch_from.ask(span, spread);
    let took = zoom_to(asked);
    orbit().pinch_from.settle(asked, took, span);
}

/// Returns the distance taken, which terrain may hold short of `dist`.
pub fn zoom_to(dist: f64) -> f64 {
    ask(IntentPatch {
        dist: Some(dist),
        ..Default::default()
    })
    .dist
}

/// Puts the pivot at its `to` at once, the eye at the drawn distance and
/// facing from it.
pub(crate) fn snap() {
    let at = resolved_to(pivot_world().unwrap_or_default());
    let cam = camera();
    let e = orbit_eye_off(orbit().dist, cam.polar, cam.azimuth);
    set_eye_world(&v3::add(&at, &e));
}

/// Eases the drawn orbit over `delta` seconds and returns whether it is
/// still moving. A camera with no eye has nothing to ease.
pub(crate) fn turn(delta: f64) -> bool {
    let Some(drawn_pivot) = pivot_world() else {
        return false;
    };
    let vs = camera_state().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_pivot, o.dist, cam.polar, cam.azimuth);

    let (at, mut moving) = ease_pivot(drawn_pivot, delta);

    // The distance eases between two that the intent holds to the camera's
    // range (state.rs `with_patch`), so it stays within it.
    let ([dist], m) = smooth_vec([o.dist], [vs.dist], CAM_DAMP, delta, EASE_EPS_U);
    o.dist = dist;
    moving |= m;

    // Under `climb`, the ground pitches the facing toward straight down
    // instead of shortening the distance. The pitch rises fast and relaxes
    // slowly while the ground bounds it.
    let polar = match settings().orbit_obstacle {
        OrbitObstacleMode::climb => climb_polar(drawn_pivot, o.dist, vs.polar, vs.azimuth),
        _ => vs.polar,
    };
    let rate = match (polar < vs.polar, polar < cam.polar) {
        (true, true) => CLIMB_RISE_DAMP,
        (true, false) => CLIMB_RELAX_DAMP,
        (false, _) => CAM_DAMP,
    };
    moving |= ease_facing(cam, polar, rate, vs.azimuth, delta);

    // The eye is written only when something moved, so a camera at rest is
    // never rewritten.
    if (at, o.dist, cam.polar, cam.azimuth) != was {
        let e = orbit_eye_off(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&v3::add(&at, &e));
    }
    moving
}
