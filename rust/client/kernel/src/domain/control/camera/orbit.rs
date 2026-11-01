// pxl-kernel/domain/control/camera/orbit.rs

//! The orbit camera: the eye moves over a sphere about the pivot, the
//! radius being the zoom. The controls write its `to`; `turn` brings what
//! is drawn to it (domain/smooth.rs). Everything that reads the world reads the
//! drawn state.
//!
//! The controls are written through one funnel (`ask`), where terrain
//! answers whether a state puts the eye in the ground
//! (orbit/collision.rs `allowed_view`), and the camera setting decides
//! what follows: through ignores the ground, stop takes a control as far as
//! it gets, and climb keeps what was asked and draws it climbed
//! (`climbed_view`): the pitch raised over what stands in the way, and
//! under a roof over the pivot the eye short of it.

pub mod collision;

use pxl_spec::space::coord::q3::Q3;

use pxl_spec::space::coord::v3::{self, V3};
use xpute_kit::math::scalar::clamp;

use crate::domain::control::camera::controller::{
    camera, facing_dir, facing_domain, pivot, pivot_world, set_eye_world,
};
use crate::domain::control::camera::gesture::{spread, Anchor};
use crate::domain::control::camera::state::{camera_state, Intent, IntentPatch};
use crate::domain::control::navigation::support::resolve_to;
use crate::domain::control::param;
use crate::domain::control::param::DOLLY_GAIN;
use crate::domain::control::param::{CAM_DAMP, ROT_DAMP};
use crate::domain::control::param::{
    CLIMB_RELAX_DAMP, CLIMB_RISE_DAMP, ORBIT_AZIMUTH_EPS, ORBIT_DIST_EPS, ORBIT_PIVOT_EPS,
    ORBIT_POLAR_EPS,
};
use crate::domain::control::param::{MAX_DIST, MIN_DIST};
use crate::domain::param::POS_DAMP;
use crate::domain::smooth::{smooth_angle, smooth_vec};
use crate::domain::view::setting::store::{settings_store, OrbitObstacleMode};
use collision::{allowed_view, climbed_view};
use xpute_guest::global::Global;

pub struct OrbitState {
    pub dist: f64,
    /// None until first put.
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

/// Gated by terrain where the controls are written, so turning back moves at
/// once. `climb` limits only a tilt asked: a zoom it answers by pitching the
/// drawn facing, which would stick if written into the controls.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_state();
    let mut asked = c.with_patch(partial);
    if partial.polar.is_some() && settings_store().orbit_obstacle == OrbitObstacleMode::climb {
        asked.polar = climbed_view(asked).polar;
    }
    c.commit_intent(allowed_view(asked, c.intent));
    c.intent
}

/// Against what is drawn for the distance asked, which a roof may hold short
/// of it (`climbed_view`).
pub fn zoom_settling() -> bool {
    (drawn(camera_state().intent).dist - orbit().dist).abs() > ORBIT_DIST_EPS
}

/// What the orbit is drawn toward for `view`, asked: in `climb` climbed
/// (`climbed_view`), else as asked.
fn drawn(view: Intent) -> Intent {
    match settings_store().orbit_obstacle {
        OrbitObstacleMode::climb => climbed_view(view),
        _ => view,
    }
}

/// `delta_y` in CSS pixels.
pub fn wheel(delta_y: f64) {
    zoom_by(libm::exp(delta_y * DOLLY_GAIN));
}

/// Under one zooms closer.
pub fn zoom_by(factor: f64) {
    zoom_to(camera_state().intent.dist * factor);
}

pub fn pinch_begin(span: f64) {
    orbit().pinch_from = Anchor::new(camera_state().intent.dist, span);
}

/// Spreading the fingers zooms in.
pub fn pinch(span: f64) {
    let asked = orbit().pinch_from.ask(span, spread);
    let took = zoom_to(asked);
    orbit().pinch_from.settle(asked, took, span);
}

/// Returns the distance taken.
pub fn zoom_to(dist: f64) -> f64 {
    ask(IntentPatch {
        dist: Some(dist),
        ..Default::default()
    })
    .dist
}

pub(crate) fn pivot_to() -> Option<Q3> {
    orbit().to.or_else(pivot)
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

/// Whether still moving; None where the camera stands nowhere.
pub(crate) fn turn(delta: f64) -> Option<bool> {
    let drawn_pivot = pivot_world()?;
    let vs = camera_state().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_pivot, o.dist, cam.polar, cam.azimuth);

    let to = resolve_to(&mut o.to, drawn_pivot);
    let (at, mut moving) = smooth_vec(drawn_pivot, to, POS_DAMP, delta, ORBIT_PIVOT_EPS);

    // `climb` pitches the facing toward straight down over what is in the
    // way, rising fast and relaxing slowly, and holds the distance short of a
    // roof over the pivot.
    let to_draw = drawn(vs);
    let ([dist], m) = smooth_vec([o.dist], [to_draw.dist], CAM_DAMP, delta, ORBIT_DIST_EPS);
    o.dist = clamp(dist, MIN_DIST, MAX_DIST);
    moving |= m;
    let polar = to_draw.polar;
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

    // Written only where something moved, so nothing is written back at rest.
    if (at, o.dist, cam.polar, cam.azimuth) != was {
        let e = orbit_eye_off(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&v3::add(&at, &e));
    }
    Some(moving)
}
