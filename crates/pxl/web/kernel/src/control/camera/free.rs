// pxl-kernel/control/camera/free.rs

//! The free camera: one controller of the observer's pose, the eye standing
//! where it is and turning in place. Its state is the eye and which way it
//! looks — a heading about Z and an angle down from vertical, as the orbit
//! looks — and nothing else: how far it looks, how high it stands over the
//! ground, is the world's to say along its forward ray (`look_point`).
//!
//! The controls it is driven by are the orbit's, taken as its own meaning
//! (`free_step`, `pan`, `rise`): the heading and the tilt are its look, a drag
//! walks it over the ground and the zoom carries it up and down — every move
//! at one speed, wherever it stands and whatever it looks at. The
//! map center is navigation's (shell/map_store.rs): a navigation command
//! carries the eye so the place stands where what it looked at stood, eased
//! within a jump and put there at once past one.

use pxl_spec::space::lod::Tau;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::pvf3::REACH;
use pxl_spec::space::pvf3::{ivec, projection};
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::{clamp, wrap};

use crate::control::camera::config::camera_cfg::free::UNITS_PER_PX;
use crate::control::camera::config::camera_cfg::smooth::{CAM_DAMP, POS_DAMP, ROT_DAMP};
use crate::control::camera::config::camera_cfg::view::{MAX_DIST, MIN_DIST};
use crate::control::camera::controller::JUMP_DISTANCES;
use crate::control::camera::controller::{active, Active};
use crate::control::camera::orbit::{damp_alpha, look_dir, look_domain, look_rotation, OrbitState};
use crate::control::camera::store::camera_store;
use crate::control::camera::terrain_gate::EYE_CLEARANCE_U;
use crate::global::Global;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, CameraCollisionMode};
use crate::world::access::{Access, Detail, Ray, FINEST};
use crate::world::change::request_world_tick;
use crate::world::frame::view::{view, view_compose, ObserverPose};

/// The free camera as drawn.
pub struct FreeState {
    /// The eye, in the view's root frame.
    pub eye: V3,
    /// Which way it looks: the heading about Z and the angle down from
    /// vertical, as the orbit's azimuth and polar.
    pub yaw: f64,
    pub pitch: f64,
}

/// What the free camera keeps of the controls between turns: the last
/// navigation command it took, and where the eye is easing to — what the
/// drag, the zoom and a navigation command within a jump move, as the
/// orbit's controls move its target and its eye follows.
struct Drive {
    navigated: u32,
    toward: Option<V3>,
}

static FREE: Global<FreeState> = Global::new();
static DRIVE: Global<Drive> = Global::new();

pub fn free() -> &'static mut FreeState {
    FREE.get(|| FreeState { eye: [0.0; 3], yaw: 0.0, pitch: 0.0 })
}

fn drive() -> &'static mut Drive {
    DRIVE.get(|| Drive { navigated: 0, toward: None })
}

/// The observer's pose the free camera makes.
pub fn free_pose(f: &FreeState) -> ObserverPose {
    ObserverPose {
        eye: f.eye,
        rotation: look_rotation(f.pitch, f.yaw),
    }
}

/// What a look from `eye` along `forward` falls on, in the root frame: the
/// first thing it meets that is resident, else the sea, else `fallback`
/// along it. On the forward ray whichever it is, so an orbit about it sees
/// what the look saw.
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

/// The free camera taking over from the orbit, the view where it was: the
/// orbit's eye, and its heading and tilt as the free camera's look.
pub fn from_orbit(o: &OrbitState) {
    let e = crate::control::camera::orbit::orbit_pose(o).eye;
    *free() = FreeState {
        eye: e,
        yaw: o.azimuth,
        pitch: o.polar + o.climb,
    };
    let d = drive();
    d.navigated = map_store().navigated;
    d.toward = None;
}

/// The orbit that sees what the free camera sees. A free pose has no pivot —
/// any point ahead on its look would make the same view — so one is chosen:
/// what the look falls on, at a distance the orbit can take, on the look
/// itself, so the eye stands where it stood and looks the way it looked.
pub fn to_orbit(f: &FreeState, fallback: f64) -> OrbitState {
    let forward = look_dir(f.pitch, f.yaw);
    let at = look_point(f.eye, forward, fallback);
    let dist = clamp((0..3).map(|k| (at[k] - f.eye[k]).powi(2)).sum::<f64>().sqrt(), MIN_DIST, MAX_DIST);
    OrbitState {
        pivot: std::array::from_fn(|k| f.eye[k] + forward[k] * dist),
        dist,
        polar: f.pitch,
        climb: 0.0,
        azimuth: f.yaw,
        placed: true,
    }
}

/// A navigation command taken, where the map center moved since the last:
/// the eye carried so the new center stands where what it looked at stood —
/// at once past a jump, eased within one. Whether it was put there at once.
pub fn navigate() -> bool {
    let d = drive();
    let nav = map_store();
    if nav.navigated == d.navigated {
        return false;
    }
    d.navigated = nav.navigated;
    let Some(center) = nav.map_center else { return false };
    let mut target = [0.0; 3];
    projection::unproject(&mut target, ivec::sub(center.i, view().origin), center.f);
    let f = free();
    let look = look_point(f.eye, look_dir(f.pitch, f.yaw), view().far);
    let by: V3 = std::array::from_fn(|k| target[k] - look[k]);
    let dest: V3 = std::array::from_fn(|k| f.eye[k] + by[k]);
    let reach = (0..3).map(|k| (f.eye[k] - look[k]).powi(2)).sum::<f64>().sqrt();
    if by[0].hypot(by[1]).hypot(by[2]) > reach * JUMP_DISTANCES {
        f.eye = dest;
        d.toward = None;
        confine(&mut f.eye);
        view_compose();
        return true;
    }
    d.toward = Some(dest);
    false
}

/// A drag walks the eye over the ground, the ground following the finger:
/// `dx`, `dy` in pixels, at the one speed (`UNITS_PER_PX`).
pub fn pan(dx: f64, dy: f64) {
    let (lx, ly) = (-dx * UNITS_PER_PX, dy * UNITS_PER_PX);
    let (s, c) = free().yaw.sin_cos();
    steer([lx * c + ly * s, -lx * s + ly * c, 0.0]);
}

/// Where the eye is easing to, moved by `by`: from where it was easing to,
/// or from the eye where it was at rest.
fn steer(by: V3) {
    let d = drive();
    let from = d.toward.unwrap_or(free().eye);
    let mut to = std::array::from_fn(|k| from[k] + by[k]);
    confine(&mut to);
    d.toward = Some(to);
    request_world_tick();
}

/// Where the free camera may stand: no higher than the orbit's eye goes, no
/// lower than the lattice reaches and within its rows north and south — the
/// one space both controllers stand in — and, where the collision policy is
/// on, a clearance over the ground under it, where that ground is known.
fn confine(eye: &mut V3) {
    let o = view().origin;
    let base = [ivec::to_real(ivec::X(o)), ivec::to_real(ivec::Y(o)), ivec::to_real(ivec::Z(o))];
    let y = (eye[1] + base[1]).clamp(REACH.min[1], REACH.max[1] - 1.0);
    let z = (eye[2] + base[2]).clamp(REACH.min[2], MAX_DIST);
    (eye[1], eye[2]) = (y - base[1], z - base[2]);
    if settings_store().camera_collision == CameraCollisionMode::through {
        return;
    }
    let mut under = Pvf3::default();
    projection::project(&mut under, eye[0] + base[0], y, 0.0);
    if let Some(ground) = Access::now().at(&under, FINEST).value() {
        eye[2] = eye[2].max(ground.top + EYE_CLEARANCE_U - base[2]);
    }
}

/// The free camera's turn, where it is the controller: its look eased
/// toward the controls' heading and tilt, a navigation command the tick's
/// input gave taken, and one within a jump eased toward.
pub fn free_step(ctx: &mut TickContext) {
    if turn(ctx.delta) == Some(true) {
        ctx.invalidate();
    }
}

/// The free camera's turn over `delta` seconds: whether it is still moving,
/// none where it is not the controller — which does nothing, and touches
/// nothing of its own.
fn turn(delta: f64) -> Option<bool> {
    if active() != Active::Free {
        return None;
    }
    let mut moving = navigate();
    let vs = camera_store().view_state;
    let f = free();
    let d = drive();

    let heading = wrap(vs.azimuth - f.yaw, -core::f64::consts::PI, core::f64::consts::PI);
    let tilt = vs.polar - f.pitch;
    (f.pitch, f.yaw) = look_domain(f.pitch + tilt * damp_alpha(CAM_DAMP, delta), f.yaw + heading * damp_alpha(ROT_DAMP, delta));
    moving |= heading.abs() > 1e-4 || tilt.abs() > 1e-4;

    // The ground under the target may have arrived since it was set: held
    // to it too, or an eye held over the ground eases toward a target under
    // it and never arrives.
    if let Some(to) = &mut d.toward {
        confine(to);
    }
    if let Some(to) = d.toward {
        let a = damp_alpha(POS_DAMP, delta);
        f.eye = std::array::from_fn(|i| f.eye[i] + (to[i] - f.eye[i]) * a);
        if (0..3).all(|i| (to[i] - f.eye[i]).abs() <= 0.0005) {
            d.toward = None;
        } else {
            moving = true;
        }
    }
    confine(&mut f.eye);
    view_compose();
    Some(moving)
}

/// The zoom as a rise, `px` in the wheel's or the pinch's pixels at the
/// speed a drag walks at: straight up or down along the world's Z, not the
/// camera's up, so a tilted look climbs as a level one does — turned as a
/// zoom out would be, the eye climbs.
pub fn rise(px: f64) {
    steer([0.0, 0.0, px * UNITS_PER_PX]);
}

/// The free camera and what it is easing to, moved with the origin.
pub fn rebase(by: V3) {
    let f = free();
    f.eye = std::array::from_fn(|k| f.eye[k] - by[k]);
    if let Some(to) = &mut drive().toward {
        *to = std::array::from_fn(|k| to[k] - by[k]);
    }
}

#[cfg(test)]
#[path = "free.test.rs"]
mod test;
