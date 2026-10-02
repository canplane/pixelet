// pxl-kernel/control/camera/orbit.rs

//! The orbit camera: the eye moves over a sphere about the map center, the
//! radius being the zoom. The controls write the target; `turn` brings what
//! is drawn to it (approach.rs). Everything that reads the world reads the
//! drawn state.

pub mod terrain_gate;

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
use crate::shell::setting::store::{settings_store, CameraCollisionMode};
use crate::world::access::support_revision;
use crate::world::frame::coord::canonical_to_local;
use crate::world::frame::view::{view, view_compose};
use terrain_gate::{allowed_pan, allowed_view, climb_polar, ViewTriple};

pub struct OrbitState {
    pub dist: f64,
    /// Canonical, so a rebase leaves it where it is. None until first put.
    to: Option<Pvf3>,
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

/// The climb samples terrain twice and was asked every ticked frame at rest;
/// it changes only with the view, the map center or `support_revision`.
struct ClimbMemo {
    dist: f64,
    polar: f64,
    azimuth: f64,
    center: Option<Pvf3>,
    revision: [u32; 2],
    polar_out: f64,
}

static MEMO: Global<ClimbMemo> = Global::new();

fn climb_polar_memo(dist: f64, polar: f64, azimuth: f64) -> f64 {
    let memo = MEMO.get(|| ClimbMemo {
        dist: f64::NAN,
        polar: f64::NAN,
        azimuth: f64::NAN,
        center: None,
        revision: [u32::MAX; 2],
        polar_out: f64::NAN,
    });
    let center = map_center();
    let revision = support_revision();
    if dist == memo.dist && polar == memo.polar && azimuth == memo.azimuth && center == memo.center && revision == memo.revision {
        return memo.polar_out;
    }
    let out = climb_polar(dist, polar, azimuth);
    *memo = ClimbMemo {
        dist,
        polar,
        azimuth,
        center,
        revision,
        polar_out: out,
    };
    out
}

/// Gated by terrain where the controls are written, so turning back moves at
/// once. `climb` limits only a tilt asked: a zoom it answers by pitching the
/// drawn look, which would stick if written into the controls.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_store();
    let mut asked = c.merged(partial);
    if partial.polar.is_some() && settings_store().camera_collision == CameraCollisionMode::climb {
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

fn drawn() -> ViewTriple {
    let cam = camera();
    ViewTriple {
        dist: orbit().dist,
        polar: cam.pitch,
        azimuth: cam.yaw,
    }
}

pub fn zoom_settling() -> bool {
    (camera_store().intent.dist - orbit().dist).abs() > DIST_EPS
}

/// `delta_y` in CSS pixels.
pub fn wheel(delta_y: f64) {
    zoom_by((delta_y * DOLLY_GAIN).exp());
}

/// Under one zooms closer.
pub fn zoom_by(factor: f64) {
    zoom_to(camera_store().intent.dist * factor);
}

pub fn pinch_begin(span: f64) {
    orbit().pinch_from = Anchor::new(camera_store().intent.dist, span);
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
    orbit().to.or_else(map_center)
}

/// Gated after the walk, since the walk moves the center's Z.
pub(crate) fn gate_pan(to: &Pvf3) -> Pvf3 {
    allowed_pan(drawn(), to)
}

pub(crate) fn put(at: Pvf3, snap: bool) {
    orbit().to = Some(at);
    if snap {
        let local = canonical_to_local(&at, view().origin);
        let (o, cam) = (orbit(), camera());
        let e = orbit_eye_offset(o.dist, cam.pitch, cam.yaw);
        set_eye_local(&std::array::from_fn(|k| local[k] + e[k]));
    }
}

/// Whether still moving; None where the camera stands nowhere.
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

    // `climb` pitches the look toward straight down instead of shortening dist:
    // rising fast and relaxing slowly while the ground bounds it.
    let polar = match settings_store().camera_collision {
        CameraCollisionMode::climb => climb_polar_memo(o.dist, vs.polar, vs.azimuth),
        _ => vs.polar,
    };
    let rate = match (polar < vs.polar, polar < cam.pitch) {
        (true, true) => CLIMB_RISE_DAMP,
        (true, false) => CLIMB_RELAX_DAMP,
        (false, _) => CAM_DAMP,
    };
    let (pitch, m) = approach_scalar(cam.pitch, polar, rate, delta, POLAR_EPS);
    moving |= m;
    let (yaw, m) = approach_angle(cam.yaw, vs.azimuth, ROT_DAMP, delta, AZIMUTH_EPS);
    moving |= m;
    (cam.pitch, cam.yaw) = look_domain(pitch, yaw);

    // Written only where something moved, so nothing is written back at rest.
    if (center, o.dist, cam.pitch, cam.yaw) != was {
        let e = orbit_eye_offset(o.dist, cam.pitch, cam.yaw);
        set_eye_local(&std::array::from_fn(|k| center[k] + e[k]));
    }
    view_compose();
    Some(moving)
}
