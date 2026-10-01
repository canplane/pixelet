// pxl-kernel/control/camera/orbit.rs

//! The orbit camera. It turns the camera (controller.rs `Camera`) about a
//! point its radius ahead of the eye along the look — the map center —
//! keeping that point where it is: the eye moves over the sphere about it,
//! and the radius is the zoom. A drag moves the map center over the ground
//! (`pan`).
//!
//! What the controls ask of it and what is drawn are kept apart: the
//! controls write the target — the look and the distance in the camera
//! store's intent, and the map center where the hand has put it — and
//! `turn` brings what is drawn to it, every channel by the one law
//! (approach.rs). The drawn state is the radius here and the eye and the
//! look in the camera; the map center drawn is derived from them. The target is the controls' alone: what draws
//! or reads the world reads the drawn state.

use pxl_spec::space::pvf3::pvf3::Pvf3;

use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::clamp;

use crate::control::camera::approach::{approach_angle, approach_scalar, approach_vec};
use crate::control::camera::config::camera_cfg;
use crate::control::camera::config::camera_cfg::input::DOLLY_GAIN;
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MIN_DIST};
use crate::control::camera::controller::{camera, center_local, look_dir, look_domain, map_center, set_eye_local};
use crate::control::camera::gesture::{spread, Anchor};
use crate::control::camera::store::{camera_store, Intent, IntentPatch};
use crate::global::Global;
use crate::world::frame::coord::canonical_to_local;
use crate::world::frame::view::{view, view_compose};

/// The orbit's own state, as drawn: the radius, how far ahead of the eye the
/// map center stands.
pub struct OrbitState {
    pub dist: f64,
    /// Where the controls have put the map center, canonical, so a rebase
    /// leaves it where it is: what the drawn center comes to. None until
    /// the camera is first put anywhere.
    to: Option<Pvf3>,
    /// A pinch's anchor: the distance the controls asked and the fingers'
    /// spread as it began, or where it was last held to (gesture.rs). A
    /// snapshot of the gesture, not a second distance.
    pinch_from: Anchor,
}

static ORBIT: Global<OrbitState> = Global::new();

pub fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: camera_cfg::view::DEFAULT_DIST,
        to: None,
        pinch_from: Anchor::new(camera_cfg::view::DEFAULT_DIST, 1.0),
    })
}

/// The eye's offset from what it looks at, `dist` back along the look.
pub fn orbit_eye_offset(dist: f64, polar: f64, azimuth: f64) -> V3 {
    look_dir(polar, azimuth).map(|v| -v * dist)
}

// How close each channel comes before it takes its target whole
// (approach.rs): world units for the center and the radius, radians for the
// angles — all sub-pixel at any distance the view is looked across.
const CENTER_EPS: f64 = 0.001;
const DIST_EPS: f64 = 0.001;
const POLAR_EPS: f64 = 0.0001;
const AZIMUTH_EPS: f64 = 0.0001;

/// The controls ask `partial` of the orbit: taken as asked, as the free
/// camera's are.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_store();
    c.commit_intent(c.merged(partial));
    c.intent
}

/// Whether the radius is still coming to the distance the controls ask.
pub fn zoom_settling() -> bool {
    (camera_store().intent.dist - orbit().dist).abs() > DIST_EPS
}

/// A wheel's turn, `delta_y` in CSS pixels: the distance scaled by it, away
/// as the wheel turns toward the hand's back.
pub fn wheel(delta_y: f64) {
    zoom_by((delta_y * DOLLY_GAIN).exp());
}

/// The zoom, `factor` of the distance the controls ask: under one closer.
pub fn zoom_by(factor: f64) {
    zoom_to(camera_store().intent.dist * factor);
}

/// Two fingers came down `span` pixels apart.
pub fn pinch_begin(span: f64) {
    orbit().pinch_from = Anchor::new(camera_store().intent.dist, span);
}

/// The fingers `span` pixels apart: the distance the pinch began at, by as
/// much as they have come together since — spreading them zooms in.
pub fn pinch(span: f64) {
    let asked = orbit().pinch_from.ask(span, spread);
    let took = zoom_to(asked);
    orbit().pinch_from.settle(asked, took, span);
}

/// The zoom to `dist`, as far as the limits and the ground let it: the
/// distance it took.
pub fn zoom_to(dist: f64) -> f64 {
    ask(IntentPatch {
        dist: Some(dist),
        ..Default::default()
    })
    .dist
}

/// The map center the controls have put, not yet drawn.
pub(crate) fn target_center() -> Option<Pvf3> {
    orbit().to.or_else(map_center)
}

/// The map center's target put at `at`: where a place said outright puts
/// it, and with `snap` the drawn center there at once.
pub(crate) fn put(at: Pvf3, snap: bool) {
    orbit().to = Some(at);
    if snap {
        let local = canonical_to_local(&at, view().origin);
        let (o, cam) = (orbit(), camera());
        let e = orbit_eye_offset(o.dist, cam.pitch, cam.yaw);
        set_eye_local(&std::array::from_fn(|k| local[k] + e[k]));
    }
}

/// The orbit's turn over `delta` seconds: the map center, the radius and
/// the look each brought toward their targets — and the eye put where they
/// have it about the map center. Whether it is still moving; none
/// where the camera stands nowhere.
pub(crate) fn turn(delta: f64) -> Option<bool> {
    let drawn_center = center_local()?;
    let vs = camera_store().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_center, o.dist, cam.pitch, cam.yaw);

    let to = o.to.map_or(drawn_center, |at| canonical_to_local(&at, view().origin));
    let (center, mut moving) = approach_vec(drawn_center, to, POS_DAMP, delta, CENTER_EPS);

    let (dist, m) = approach_scalar(o.dist, vs.dist, CAM_DAMP, delta, DIST_EPS);
    o.dist = clamp(dist, MIN_DIST, MAX_DIST);
    moving |= m;

    let (pitch, m) = approach_scalar(cam.pitch, vs.polar, CAM_DAMP, delta, POLAR_EPS);
    moving |= m;
    let (yaw, m) = approach_angle(cam.yaw, vs.azimuth, ROT_DAMP, delta, AZIMUTH_EPS);
    moving |= m;
    (cam.pitch, cam.yaw) = look_domain(pitch, yaw);

    // The eye is written only where the center, the radius or the look
    // moved: at rest nothing is derived and written back.
    if (center, o.dist, cam.pitch, cam.yaw) != was {
        let e = orbit_eye_offset(o.dist, cam.pitch, cam.yaw);
        set_eye_local(&std::array::from_fn(|k| center[k] + e[k]));
    }
    view_compose();
    Some(moving)
}
