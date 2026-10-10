// pxl-kernel/control/camera/orbit.rs

//! The orbit camera: the eye moves over a sphere about the focus, the
//! radius being the zoom. The controls write the target; `turn` brings what
//! is drawn to it (approach.rs). Everything that reads the world reads the
//! drawn state.

pub mod terrain_gate;

use pxl_spec::space::coord::pvf3::pvf3::Pvf3;

use pxl_spec::space::coord::v3::{add3, V3};
use xpute_core::math::scalar::clamp;

use crate::approach::{approach_angle, approach_scalar, approach_vec};
use crate::control::camera::controller::{camera, focus, focus_world, look_dir, look_domain, set_eye_world};
use crate::control::camera::gesture::{spread, Anchor};
use crate::control::camera::state::{camera_state, Intent, IntentPatch};
use crate::control::navigation::support::target;
use crate::param::DOLLY_GAIN;
use crate::param::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::param::{MAX_DIST, MIN_DIST};
use crate::setting::store::{settings_store, OrbitObstacleMode};
use crate::view::frame::pose::view_compose;
use terrain_gate::{allowed_view, climb_polar, ViewTriple};
use xpute_guest::global::Global;

pub struct OrbitState {
    pub dist: f64,
    /// None until first put.
    to: Option<Pvf3>,
    pinch_from: Anchor,
}

static ORBIT: Global<OrbitState> = Global::new();

pub fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: crate::param::DEFAULT_DIST,
        to: None,
        pinch_from: Anchor::new(crate::param::DEFAULT_DIST, 1.0),
    })
}

pub fn orbit_eye_offset(dist: f64, polar: f64, azimuth: f64) -> V3 {
    look_dir(polar, azimuth).map(|v| -v * dist)
}

// Sub-pixel at any distance the view is looked across.
const CENTER_EPS: f64 = 0.001;
const DIST_EPS: f64 = 0.001;
const POLAR_EPS: f64 = 0.0001;
const AZIMUTH_EPS: f64 = 0.0001;

/// Faster than the controls ease: late to rise is the eye briefly inside a hill.
const CLIMB_RISE_DAMP: f64 = 20.0;

/// Slower than the controls ease, so passing over a ridge does not bob the view.
const CLIMB_RELAX_DAMP: f64 = 5.0;

/// Gated by terrain where the controls are written, so turning back moves at
/// once. `climb` limits only a tilt asked: a zoom it answers by pitching the
/// drawn look, which would stick if written into the controls.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_state();
    let mut asked = c.merged(partial);
    if partial.polar.is_some() && settings_store().orbit_obstacle == OrbitObstacleMode::climb {
        asked.polar = climb_polar(asked.dist, asked.polar, asked.azimuth);
    }
    c.commit_intent(gate(c.intent, asked));
    c.intent
}

fn gate(from: Intent, to: Intent) -> Intent {
    let triple = |i: Intent| ViewTriple {
        dist: i.dist,
        polar: i.polar,
        azimuth: i.azimuth,
    };
    let r = allowed_view(triple(to), triple(from));
    Intent {
        dist: r.dist,
        azimuth: r.azimuth,
        polar: r.polar,
    }
}

pub fn zoom_settling() -> bool {
    (camera_state().intent.dist - orbit().dist).abs() > DIST_EPS
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

pub(crate) fn target_center() -> Option<Pvf3> {
    orbit().to.or_else(focus)
}

pub(crate) fn put(at: Pvf3, snap: bool) {
    orbit().to = Some(at);
    if snap {
        let (o, cam) = (orbit(), camera());
        let center = target(&mut o.to, focus_world().unwrap_or_default());
        let e = orbit_eye_offset(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&add3(&center, &e));
    }
}

/// Whether still moving; None where the camera stands nowhere.
pub(crate) fn turn(delta: f64) -> Option<bool> {
    let drawn_center = focus_world()?;
    let vs = camera_state().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_center, o.dist, cam.polar, cam.azimuth);

    let to = target(&mut o.to, drawn_center);
    let (center, mut moving) = approach_vec(drawn_center, to, POS_DAMP, delta, CENTER_EPS);

    let (dist, m) = approach_scalar(o.dist, vs.dist, CAM_DAMP, delta, DIST_EPS);
    o.dist = clamp(dist, MIN_DIST, MAX_DIST);
    moving |= m;

    // `climb` pitches the look toward straight down instead of shortening dist:
    // rising fast and relaxing slowly while the ground bounds it.
    let polar = match settings_store().orbit_obstacle {
        OrbitObstacleMode::climb => climb_polar(o.dist, vs.polar, vs.azimuth),
        _ => vs.polar,
    };
    let rate = match (polar < vs.polar, polar < cam.polar) {
        (true, true) => CLIMB_RISE_DAMP,
        (true, false) => CLIMB_RELAX_DAMP,
        (false, _) => CAM_DAMP,
    };
    let (polar, m) = approach_scalar(cam.polar, polar, rate, delta, POLAR_EPS);
    moving |= m;
    let (azimuth, m) = approach_angle(cam.azimuth, vs.azimuth, ROT_DAMP, delta, AZIMUTH_EPS);
    moving |= m;
    (cam.polar, cam.azimuth) = look_domain(polar, azimuth);

    // Written only where something moved, so nothing is written back at rest.
    if (center, o.dist, cam.polar, cam.azimuth) != was {
        let e = orbit_eye_offset(o.dist, cam.polar, cam.azimuth);
        set_eye_world(&add3(&center, &e));
    }
    view_compose(crate::control::camera::controller::pose());
    Some(moving)
}
