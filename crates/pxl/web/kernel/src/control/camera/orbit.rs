// pxl-kernel/control/camera/orbit.rs

//! The orbit camera: one controller of the observer's pose. Its controls —
//! a pivot, a distance, an angle down from vertical and a heading about Z —
//! are damped toward the camera store's and the map center (`orbit_step`),
//! and the pose is made from them here (`orbit_pose`): the eye at the
//! distance from the pivot, looking at it. The world stays in its own axes;
//! what turns is the observer. Nothing below the pose knows it came from an
//! orbit.

use pxl_spec::bake::format::Terrain;
use pxl_spec::space::mat4::{mat4, mat4_mul, mat4_rotation_z, Mat4};
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::{clamp, wrap};

use crate::control::camera::config::camera_cfg;
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MIN_DIST};
use crate::control::camera::controller::{active, Active, JUMP_DISTANCES};
use crate::control::camera::store::camera_store;
use crate::control::camera::terrain_gate::climb_polar;
use crate::global::Global;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, CameraCollisionMode};
use crate::world::frame::view::{view, view_compose, ObserverPose};
use crate::world::pyramid::Layer as _;

/// The orbit as drawn: its controls as its step has eased them.
pub struct OrbitState {
    /// The eye's distance from the pivot.
    pub dist: f64,
    /// The angle down from vertical the controls ask, eased.
    pub polar: f64,
    /// How much steeper than that the eye is drawn, toward straight down, to
    /// clear the ground (the collision setting's climb): never above zero,
    /// the terrain's correction apart from what the controls ask, and zero
    /// unless it is climbing.
    pub climb: f64,
    /// The heading about Z: at zero the eye stands south of the pivot and
    /// looks north.
    pub azimuth: f64,
    /// What the eye looks at: the map center as the view has eased toward
    /// it, in the view's root frame (world/frame/view.rs).
    pub pivot: V3,
    /// Whether the orbit has been placed: until the map center first comes
    /// it stands nowhere, and then it is put where every control asks at
    /// once — the pivot, the distance, the tilt and the heading together —
    /// and eases from there.
    pub placed: bool,
}

static ORBIT: Global<OrbitState> = Global::new();

pub fn orbit() -> &'static mut OrbitState {
    ORBIT.get(|| OrbitState {
        dist: camera_cfg::view::DEFAULT_DIST,
        polar: 0.0,
        climb: 0.0,
        azimuth: 0.0,
        pivot: [0.0; 3],
        placed: false,
    })
}

// ---- the look, in spherical coordinates about the ground's normal ----
//
// Which way a camera looks is a point of the sphere of directions, taken in
// spherical coordinates about the world's Z: θ the angle from straight down
// and φ the heading, from north toward east. θ runs over [0, π] — straight
// down to straight up — and stops there, as the coordinate does; φ is a turn,
// [−π, π), and wraps. Every controller looks by these, and nothing else is a
// way to write a direction.
//
// The coordinates are not one to one with directions, and cannot be: at
// either pole every φ names the one direction, as spherical coordinates do.
// φ is kept there all the same, the heading the look comes back down along
// when it tilts away. Nor is a direction a camera's whole turn: a turn about
// the look itself, a roll, is left, and no rule chooses it smoothly over the
// whole sphere — at the poles the look is the world's up, and nothing is
// across from it. Roll is no control here; the camera's turn is the
// convention `look_rotation` states, under which φ at a pole turns the screen
// about the look rather than the look.

/// A look's coordinates brought into their domain: θ within straight down
/// and straight up, φ wrapped to a turn.
pub fn look_domain(theta: f64, phi: f64) -> (f64, f64) {
    (theta.clamp(0.0, core::f64::consts::PI), wrap(phi, -core::f64::consts::PI, core::f64::consts::PI))
}

/// The unit direction a look at (θ, φ) points along.
pub fn look_dir(theta: f64, phi: f64) -> V3 {
    let (st, ct) = (libm::sin(theta), libm::cos(theta));
    let (sp, cp) = (libm::sin(phi), libm::cos(phi));
    [st * sp, st * cp, -ct]
}

/// The turn from the world's axes to the camera's for a look at (θ, φ), the
/// camera looking along `look_dir`: the world turned by the heading, then
/// about the camera's right by θ — straight down at zero with the heading up
/// the screen, the world's up toward the screen's up between the poles. A
/// rotation at every (θ, φ), the poles included, where it is the heading's
/// turn of the screen alone.
pub fn look_rotation(theta: f64, phi: f64) -> Mat4 {
    let mut heading = mat4();
    mat4_rotation_z(&mut heading, phi);
    let (s, c) = (libm::sin(theta), libm::cos(theta));
    let mut tilt = mat4();
    (tilt[5], tilt[6], tilt[9], tilt[10]) = (c, -s, s, c);
    let mut rotation: Mat4 = mat4();
    mat4_mul(&mut rotation, &tilt, &heading);
    rotation
}

/// The eye's offset from what it looks at, `dist` back along the look.
pub fn orbit_eye_offset(dist: f64, polar: f64, azimuth: f64) -> V3 {
    look_dir(polar, azimuth).map(|v| -v * dist)
}

/// The observer's pose the orbit makes: the eye at its offset from the
/// pivot, at the tilt the controls ask and the climb steepens, looking at it.
pub fn orbit_pose(o: &OrbitState) -> ObserverPose {
    let polar = o.polar + o.climb;
    let e = orbit_eye_offset(o.dist, polar, o.azimuth);
    ObserverPose {
        eye: [o.pivot[0] + e[0], o.pivot[1] + e[1], o.pivot[2] + e[2]],
        rotation: look_rotation(polar, o.azimuth),
    }
}

/// Exponential-decay ease-toward-target factor for one frame of `delta`
/// seconds at rate `k` (bigger k = snappier convergence). Shared by every
/// damped camera value in this module tree (the orbit's in `orbit_step`, the
/// free camera's in free.rs) so they all converge with the same
/// characteristic curve — apply as
/// `current += (target - current) * damp_alpha(k, delta)`.
pub fn damp_alpha(k: f64, delta: f64) -> f64 {
    1.0 - libm::exp(-k * delta)
}

// The tick runs another frame only when something asks (sched/tick.rs) —
// this damping is the main thing asking while the camera is settling after a
// drag/zoom, and needs to stop once it's close enough that a further frame
// wouldn't look any different, or it would ask forever and the view would
// never be idle. World units for dist and the pivot, radians for the angles —
// all small enough that the remaining error is sub-pixel at any zoom level in
// view.
const DIST_EPS: f64 = 0.001;
const POLAR_EPS: f64 = 0.0001;
const AZIMUTH_EPS: f64 = 0.0001;
const POS_EPS: f64 = 0.0005;

/// How quickly the climb's own contribution follows what terrain asks for.
///
/// Elevation is read per cell, so the answer steps rather than slides. Rising
/// is the faster of the two: late to rise means the eye is briefly inside a
/// hill, late to relax means the view sits steeper than it needs to for a
/// moment longer.
const CLIMB_RISE_DAMP: f64 = 20.0;
const CLIMB_RELAX_DAMP: f64 = 5.0;
const CLIMB_EPS: f64 = 0.0005;

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
    let center = map_store().map_center;
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

/// Where the orbit's pivot is to stand for the map center, in the view's
/// root frame, and the orbit put there at once where it is not placed yet —
/// every control at once — or the map center is a jump away — a search, a
/// locate — which is what the second of the pair says. None before there is
/// a map center.
pub(crate) fn snap_if_discontinuous() -> Option<(V3, bool)> {
    let map_center = map_store().map_center?;
    let o = orbit();
    let vs = camera_store().view_state;
    let mut target = [0.0; 3];
    projection::unproject(&mut target, ivec::sub(map_center.i, view().origin), map_center.f);
    let gap = (target[0] - o.pivot[0]).hypot(target[1] - o.pivot[1]).hypot(target[2] - o.pivot[2]);
    if o.placed && gap <= vs.dist * JUMP_DISTANCES {
        return Some((target, false));
    }
    if !o.placed {
        (o.dist, o.polar, o.climb, o.azimuth, o.placed) = (vs.dist, vs.polar, 0.0, vs.azimuth, true);
    }
    o.pivot = target;
    view_compose();
    Some((target, true))
}

/// The orbit's turn, while it is the controller: its distance and tilt eased
/// toward the controls, the climb toward what the ground asks, the heading
/// the short way round toward the controls', and the pivot toward the map
/// center — or put there at once on a jump the tick's own input made, which
/// asks for the turn after, whose effects observe it.
pub fn orbit_step(ctx: &mut TickContext) {
    if active() != Active::Orbit {
        return;
    }
    let Some((target, snapped)) = snap_if_discontinuous() else { return };
    if snapped {
        ctx.invalidate();
        return;
    }
    let vs = camera_store().view_state;
    let o = orbit();
    let mut moving = false;

    let alpha = damp_alpha(CAM_DAMP, ctx.delta);
    o.dist = clamp(o.dist + (vs.dist - o.dist) * alpha, MIN_DIST, MAX_DIST);
    o.polar += (vs.polar - o.polar) * alpha;
    moving |= (vs.dist - o.dist).abs() > DIST_EPS || (vs.polar - o.polar).abs() > POLAR_EPS;
    if (vs.dist - o.dist).abs() <= DIST_EPS {
        camera_store().wheel_zooming = false;
    }

    // `climb` grants the control and answers terrain by pitching the view
    // toward straight down instead. dist stays exactly what the scale bar
    // reads; only the angle gives way, and it comes back on its own once the
    // ground does. What is smoothed here is the climb's own contribution,
    // never the pitch itself: polar is already damped alongside dist just
    // above, and running the absolute angle through a second lag gave tilt
    // an inertia rotation does not have. Outside climb mode the offset is
    // zero and the drawn pitch is exactly the damped control.
    let want = if settings_store().camera_collision == CameraCollisionMode::climb {
        climb_polar_memo(o.dist, o.polar, vs.azimuth) - o.polar
    } else {
        0.0
    };
    let k = if want < o.climb { CLIMB_RISE_DAMP } else { CLIMB_RELAX_DAMP };
    o.climb += (want - o.climb) * damp_alpha(k, ctx.delta);
    moving |= (want - o.climb).abs() > CLIMB_EPS;

    let turn = wrap(vs.azimuth - o.azimuth, -core::f64::consts::PI, core::f64::consts::PI);
    let a = damp_alpha(ROT_DAMP, ctx.delta);
    (o.polar, o.azimuth) = look_domain(o.polar, o.azimuth + turn * a);
    moving |= (turn * (1.0 - a)).abs() > AZIMUTH_EPS;

    let a = damp_alpha(POS_DAMP, ctx.delta);
    o.pivot = std::array::from_fn(|k| o.pivot[k] + (target[k] - o.pivot[k]) * a);
    moving |= (0..3).any(|k| (target[k] - o.pivot[k]).abs() > POS_EPS);

    view_compose();
    if moving {
        ctx.invalidate();
    }
}

#[cfg(test)]
#[path = "orbit.test.rs"]
mod test;
