// pxl-kernel/control/camera/orbit.rs

//! The orbit camera. It turns the camera (controller.rs `Camera`) about a
//! point its radius ahead of the eye along the look — the map center —
//! keeping that point where it is: the eye moves over the sphere about it,
//! and the radius is the zoom. A drag walks the map center (controller.rs
//! `pan`).
//!
//! What the controls ask of it and what is drawn are kept apart: the
//! controls write the target — the look and the distance in the camera
//! store's intent, and the map center where the hand has put it, where the
//! walk stands it (controller.rs `pan`) — and `turn` brings what is
//! drawn to it, every channel by the one law (approach.rs). The drawn state
//! is the radius here and the eye and the look in the camera; the map center
//! drawn is derived from them. The target is the controls' alone: what draws
//! or reads the world reads the drawn state.

pub mod terrain_gate;

use pxl_spec::space::pvf3::pvf3::Pvf3;

use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::clamp;

use crate::control::camera::approach::{approach_angle, approach_scalar, approach_vec};
use crate::control::camera::config::camera_cfg;
use crate::control::camera::config::camera_cfg::input::DOLLY_GAIN;
use crate::control::camera::approach::{approach_angle, approach_scalar, approach_vec};
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MIN_DIST};
use crate::control::camera::controller::{camera, center_local, look_dir, look_domain, map_center, set_eye_local};
use crate::control::camera::gesture::{spread, Anchor};
use crate::control::camera::store::{camera_store, Intent, IntentPatch};
use crate::control::navigation::support::target;
use crate::global::Global;
use crate::shell::setting::store::{settings_store, OrbitObstacleMode};
use crate::world::frame::view::{view, view_compose};
use crate::world::observation::heights_revision;
use terrain_gate::{allowed_view, climb_polar, ViewTriple};

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

/// How quickly the look steepens where the ground asks it to (the collision
/// setting's climb): faster than the controls ease, since late to rise is
/// the eye briefly inside a hill.
const CLIMB_RISE_DAMP: f64 = 20.0;

/// How quickly the look comes back up as the ground lets it, while the
/// ground still bounds it: slower than the controls ease, so passing over a
/// ridge does not bob the view.
const CLIMB_RELAX_DAMP: f64 = 5.0;

/// climb_polar's answer for the last inputs: the climb samples the terrain
/// twice through the page tables and page math, and every ticked frame asked
/// it again with the view at rest. The answer changes only with the view, the
/// map center, or what the eye stands over changing — a page arriving, which
/// is also when ground not known yet becomes known, or an object placed or
/// carved (world/access.rs `support_revision`).
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

/// The controls ask `partial` of the orbit: the intent as far from where the
/// controls stood as terrain lets the orbit go (terrain_gate.rs
/// `allowed_view`), and a tilt asked no shallower than `climb` can clear the
/// ground — taken where the controls are written, so what they hold is where
/// the orbit can stand and turning back moves it at once. What it took.
///
/// `climb` limits only a tilt asked: a zoom it answers by pitching the drawn
/// look (`turn`), which comes back as the ground does, and writing that
/// into the controls would keep it.
pub fn ask(partial: IntentPatch) -> Intent {
    let c = camera_store();
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

/// The map center the controls have put, not yet drawn.
pub(crate) fn target_center() -> Option<Pvf3> {
    orbit().to.or_else(map_center)
}

pub(crate) fn put(at: Pvf3, snap: bool) {
    orbit().to = Some(at);
    if snap {
        let (o, cam) = (orbit(), camera());
        let local = target(&mut o.to, center_local().unwrap_or_default(), view().origin);
        let e = orbit_eye_offset(o.dist, cam.polar, cam.azimuth);
        set_eye_local(&std::array::from_fn(|k| local[k] + e[k]));
    }
}

/// The orbit's turn over `delta` seconds: the map center, the radius and
/// the look each brought toward their targets — the tilt no shallower than
/// the ground allows where the collision setting climbs — and the eye put
/// where they have it about the map center. Whether it is still moving; none
/// where the camera stands nowhere.
pub(crate) fn turn(delta: f64) -> Option<bool> {
    let drawn_center = center_local()?;
    let vs = camera_store().intent;
    let (o, cam) = (orbit(), camera());
    let was = (drawn_center, o.dist, cam.polar, cam.azimuth);

    let to = target(&mut o.to, drawn_center, view().origin);
    let (center, mut moving) = approach_vec(drawn_center, to, POS_DAMP, delta, CENTER_EPS);

    let (dist, m) = approach_scalar(o.dist, vs.dist, CAM_DAMP, delta, DIST_EPS);
    o.dist = clamp(dist, MIN_DIST, MAX_DIST);
    moving |= m;

    // `climb` pitches the look toward straight down instead of shortening dist:
    // rising fast and relaxing slowly while the ground bounds it.
    let polar = match settings_store().orbit_obstacle {
        OrbitObstacleMode::climb => climb_polar_memo(o.dist, vs.polar, vs.azimuth),
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
        set_eye_local(&std::array::from_fn(|k| center[k] + e[k]));
    }
    view_compose();
    Some(moving)
}
