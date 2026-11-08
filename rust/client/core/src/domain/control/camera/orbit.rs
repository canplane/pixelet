// pxl-core/domain/control/camera/orbit.rs

//! The orbit camera. The eye moves on a sphere about the pivot, and the
//! sphere's radius is the zoom. The controls write what the camera is to
//! reach: the intent (state.rs) and the pivot's `to`. `turn` eases the drawn
//! camera toward them (domain/smooth.rs). Everything that reads the world
//! reads the drawn camera.
//!
//! Every control is written through one function, `ask`. There the terrain
//! answers whether a state puts the eye in the ground (orbit/collision.rs
//! `allowed_view`), and the obstacle setting decides what follows. With
//! `through`, the ground is ignored. With `stop`, a control goes as far as
//! it can. With `climb`, the drawn pitch rises instead (`climb_polar`).
//! Where the way of a control still hits something, a zoom in goes through
//! it (`dive`) and any other control stops (`Step`).

pub mod collision;

use pxl_spec::space::coord::q3::Q3;

use pxl_spec::space::coord::v3::{self, V3};

use crate::domain::control::camera::gesture::{spread, Anchor};
use crate::domain::control::camera::state::{camera_state, camera_state_mut, Intent, IntentPatch};
use crate::domain::control::camera::{camera, facing_dir, facing_domain, set_eye_world};
use crate::domain::control::param;
use crate::domain::control::param::DOLLY_GAIN;
use crate::domain::control::param::{CAM_DAMP, ROT_DAMP};
use crate::domain::control::param::{
    CLIMB_RELAX_DAMP, CLIMB_RISE_DAMP, ORBIT_AZIMUTH_EPS, ORBIT_DIST_EPS, ORBIT_PIVOT_EPS,
    ORBIT_POLAR_EPS,
};
use crate::domain::control::param::{MAX_DIST, MIN_DIST};
use crate::domain::control::support::resolve_to;
use crate::domain::param::POS_DAMP;
use crate::domain::setting::store::{settings, OrbitObstacleMode};
use crate::domain::smooth::{smooth_angle, smooth_vec};
use crate::domain::view::frame::coord::world_to_canonical;
use collision::{allowed_view, climb_polar};
use xpute_core::global::Global;
use xpute_kit::math::scalar::clamp;

pub struct OrbitState {
    pub dist: f64,
    /// Where the pivot goes to, as a canonical point. None until it is first
    /// put.
    to: Option<Q3>,
    pinch_from: Anchor,
}

static ORBIT: Global<OrbitState> = Global::new();

pub fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: param::DEFAULT_DIST,
        to: None,
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
/// turns back moves the camera at once. Under `climb`, only an asked tilt is
/// limited here. A zoom is answered by pitching the drawn facing in `turn`
/// instead, because a pitch written into the intent would stay after the
/// ground no longer asks for it.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_state_mut();
    let mut asked = c.with_patch(partial);
    if partial.polar.is_some() && settings().orbit_obstacle == OrbitObstacleMode::climb {
        if let Some(at) = pivot_world() {
            asked.polar = climb_polar(at, asked.dist, asked.polar, asked.azimuth);
        }
    }
    c.commit_intent(allowed_view(asked, c.intent));
    c.intent
}

pub fn zoom_settling() -> bool {
    (camera_state().intent.dist - orbit().dist).abs() > ORBIT_DIST_EPS
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

pub(crate) fn pivot_to() -> Option<Q3> {
    orbit()
        .to
        .or_else(|| pivot_world().map(|p| world_to_canonical(&p)))
}

pub(crate) fn put(at: Q3, snap: bool) {
    orbit().to = Some(at);
    if snap {
        let (o, cam) = (orbit(), camera());
        let at = resolve_to(&mut o.to, pivot_world().unwrap_or_default());
        let e = orbit_eye_off(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&v3::add(&at, &e));
    }
}

/// Eases the drawn orbit over `delta` seconds and returns whether it is
/// still moving. None when the camera has no eye.
pub(crate) fn turn(delta: f64) -> Option<bool> {
    let drawn_pivot = pivot_world()?;
    let vs = camera_state().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_pivot, o.dist, cam.polar, cam.azimuth);

    let to = resolve_to(&mut o.to, drawn_pivot);
    let (at, mut moving) = smooth_vec(drawn_pivot, to, POS_DAMP, delta, ORBIT_PIVOT_EPS);

    let ([dist], m) = smooth_vec([o.dist], [vs.dist], CAM_DAMP, delta, ORBIT_DIST_EPS);
    o.dist = clamp(dist, MIN_DIST, MAX_DIST);
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
    let ([polar], m) = smooth_vec([cam.polar], [polar], rate, delta, ORBIT_POLAR_EPS);
    moving |= m;
    let (azimuth, m) = smooth_angle(cam.azimuth, vs.azimuth, ROT_DAMP, delta, ORBIT_AZIMUTH_EPS);
    moving |= m;
    (cam.polar, cam.azimuth) = facing_domain(polar, azimuth);

    // The eye is written only when something moved, so a camera at rest is
    // never rewritten.
    if (at, o.dist, cam.polar, cam.azimuth) != was {
        let e = orbit_eye_off(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&v3::add(&at, &e));
    }
    Some(moving)
}
