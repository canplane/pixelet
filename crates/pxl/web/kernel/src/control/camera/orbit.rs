// pxl-kernel/control/camera/orbit.rs

//! The orbit camera. It turns the camera (controller.rs `Camera`) about a
//! point its radius ahead of the eye along the look — the map center —
//! keeping that point where it is: the eye moves over the
//! sphere about it, and the radius, eased toward the controls' distance
//! (`turn`), is the zoom. A drag moves the map center over the ground
//! (`pan`). Its own state is the radius; the
//! eye and the look are the camera's.

pub mod terrain_gate;

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::pvf3::pvf3::Pvf3;

use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::{clamp, wrap};

use crate::control::camera::config::camera_cfg;
use crate::control::camera::config::camera_cfg::input::DOLLY_GAIN;
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MIN_DIST};
use crate::control::camera::controller::{camera, center_local, damp_alpha, look_dir, look_domain, map_center, set_eye_local, spend_travel, units_per_px_at};
use crate::control::camera::gesture::{spread, Anchor};
use crate::control::camera::store::{camera_store, Intent, IntentPatch};
use crate::global::Global;
use crate::layer::terrain::ground_snap::ground_or_raw;
use crate::shell::setting::store::{settings_store, CameraCollisionMode};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::{canonical_to_local, local_world_to_canonical};
use crate::world::frame::view::{view, view_compose};
use crate::world::pyramid::Layer as _;
use terrain_gate::{allowed_pan, allowed_view, climb_polar, ViewTriple};

/// The orbit's own state, as drawn: the radius, how far ahead of the eye the
/// map center stands.
pub struct OrbitState {
    pub dist: f64,
    /// A pinch's anchor: the distance the controls asked and the fingers'
    /// spread as it began, or where it was last held to (gesture.rs). A
    /// snapshot of the gesture, not a second distance.
    pinch_from: Anchor,
}

static ORBIT: Global<OrbitState> = Global::new();

pub fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: camera_cfg::view::DEFAULT_DIST,
        pinch_from: Anchor::new(camera_cfg::view::DEFAULT_DIST, 1.0),
    })
}

/// The eye's offset from what it looks at, `dist` back along the look.
pub fn orbit_eye_offset(dist: f64, polar: f64, azimuth: f64) -> V3 {
    look_dir(polar, azimuth).map(|v| -v * dist)
}

// The tick runs another frame only when something asks (sched/tick.rs) —
// this damping is the main thing asking while the camera is settling after a
// drag/zoom, and needs to stop once it's close enough that a further frame
// wouldn't look any different, or it would ask forever and the view would
// never be idle. World units for dist, radians for the angles —
// all small enough that the remaining error is sub-pixel at any zoom level in
// view.
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
/// map center, or a page arriving — the terrain pyramid's revision moves for
/// the last, which is also when ground not known yet becomes known.
struct ClimbMemo {
    dist: f64,
    polar: f64,
    azimuth: f64,
    center: Option<Pvf3>,
    revision: u32,
    polar_out: f64,
}

static MEMO: Global<ClimbMemo> = Global::new();

fn climb_polar_memo(dist: f64, polar: f64, azimuth: f64) -> f64 {
    let memo = MEMO.get(|| ClimbMemo {
        dist: f64::NAN,
        polar: f64::NAN,
        azimuth: f64::NAN,
        center: None,
        revision: u32::MAX,
        polar_out: f64::NAN,
    });
    let center = map_center();
    let revision = Terrain::pyramid().revision();
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

/// The orbit as drawn, as the terrain gate reads a view.
fn drawn() -> ViewTriple {
    let cam = camera();
    ViewTriple {
        dist: orbit().dist,
        polar: cam.pitch,
        azimuth: cam.yaw,
    }
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

/// A drag moves the map center over the ground, the ground following the
/// finger at the distance the view looks across, over the turns to come
/// (`walk`).
pub fn pan(dx: f64, dy: f64) {
    if map_center().is_none() {
        return;
    }
    let units_per_px = units_per_px_at(camera_store().intent.dist);
    let (lx, ly) = (-dx * units_per_px, dy * units_per_px);
    let (s, c) = camera().yaw.sin_cos();
    let travel = &mut camera_store().travel;
    travel[0] += lx * c + ly * s;
    travel[1] += -lx * s + ly * c;
    request_world_tick();
}

/// The map center, at `center` in the root frame, moved by this turn's
/// share of the hand's move: on the ground where the ground is known, and
/// stopped where the terrain gate stops it. None where there is nothing to
/// move by.
fn walk(center: V3, delta: f64) -> Option<V3> {
    let share = spend_travel(delta)?;
    let origin = view().origin;
    let mut next = Pvf3::default();
    local_world_to_canonical(&mut next, &[center[0] + share[0], center[1] + share[1], center[2]], origin);
    let desired = if settings_store().ground_snap { ground_or_raw(&next) } else { next };
    // Gated after the snap, not before: the snap moves the center's Z, and
    // the eye rides on it.
    let reached = allowed_pan(drawn(), &desired);
    Some(canonical_to_local(&reached, origin))
}

/// The orbit's turn over `delta` seconds: the map center walked by the
/// hand's move, the radius and the look eased toward the controls — the tilt
/// no shallower than the ground allows where the collision setting climbs —
/// and the eye put where they have it about the map center. Whether it is
/// still moving; none where the camera stands nowhere.
pub(crate) fn turn(delta: f64) -> Option<bool> {
    let mut center = center_local()?;
    let walked = walk(center, delta).map(|at| center = at).is_some();
    let vs = camera_store().intent;
    let (o, cam) = (orbit(), camera());
    let was = (o.dist, cam.pitch, cam.yaw);
    let mut moving = camera_store().travel != [0.0; 2];

    let alpha = damp_alpha(CAM_DAMP, delta);
    o.dist = clamp(o.dist + (vs.dist - o.dist) * alpha, MIN_DIST, MAX_DIST);
    moving |= (vs.dist - o.dist).abs() > DIST_EPS;

    // `climb` grants the controls and answers terrain by pitching the look
    // toward straight down: the tilt the look eases to is the controls',
    // bounded by the shallowest that clears the ground, and it steepens
    // quickly and relaxes slowly while the ground still bounds it. dist
    // stays exactly what the controls ask; only the angle gives way, and it
    // comes back on its own once the ground does.
    let polar = match settings_store().camera_collision {
        CameraCollisionMode::climb => climb_polar_memo(o.dist, vs.polar, vs.azimuth),
        _ => vs.polar,
    };
    let rate = match (polar < vs.polar, polar < cam.pitch) {
        (true, true) => CLIMB_RISE_DAMP,
        (true, false) => CLIMB_RELAX_DAMP,
        (false, _) => CAM_DAMP,
    };
    let pitch = cam.pitch + (polar - cam.pitch) * damp_alpha(rate, delta);
    moving |= (polar - pitch).abs() > POLAR_EPS;

    let turn = wrap(vs.azimuth - cam.yaw, -core::f64::consts::PI, core::f64::consts::PI);
    let a = damp_alpha(ROT_DAMP, delta);
    (cam.pitch, cam.yaw) = look_domain(pitch, cam.yaw + turn * a);
    moving |= (turn * (1.0 - a)).abs() > AZIMUTH_EPS;

    // The eye is written only where the center, the radius or the look
    // moved: at rest nothing is derived and written back.
    if walked || (o.dist, cam.pitch, cam.yaw) != was {
        let e = orbit_eye_offset(o.dist, cam.pitch, cam.yaw);
        set_eye_local(&std::array::from_fn(|k| center[k] + e[k]));
    }
    view_compose();
    Some(moving)
}
