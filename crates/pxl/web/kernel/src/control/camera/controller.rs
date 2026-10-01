// pxl-kernel/control/camera/controller.rs

//! Which controller makes the observer's pose: the orbit or the free camera,
//! as the settings choose, or a tracker while one holds it. Each has its own
//! state and none shares another's; what they have in common is the pose
//! they make (world/frame/view.rs `ObserverPose`), which is all the rest of
//! the kernel reads.
//!
//! A change between the orbit and the free camera keeps the view where it
//! is (free.rs `from_orbit`, `to_orbit`). A tracker overrides whichever is
//! the base while it holds the pose, which stays as it was under it and is
//! the controller again when the tracker lets go: a tracked turn can roll,
//! and neither base can take a roll as its own.

use pxl_spec::space::mat4::{mat4, Mat4};
use pxl_spec::space::v3::V3;

use crate::control::camera::free::{free, free_pose, from_orbit, look_point, to_orbit};
use crate::control::camera::orbit::{orbit, orbit_pose};
use crate::control::camera::store::camera_store;
use crate::global::Global;
use crate::sched::tick::TickContext;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, CameraMode};
use crate::world::change::request_world_tick;
use crate::world::frame::coord::local_world_to_canonical;
use crate::world::frame::view::{view, view_compose, ObserverPose};

/// How far, in camera distances, the map center may stand from where the view
/// is for the view to glide there; past it the move is a jump — a search, a
/// locate — and the view is put there at once. A drag's lag is the damping's
/// tenth of a second of travel, a fraction of a screen, and a screen is about
/// a camera distance across.
pub(crate) const JUMP_DISTANCES: f64 = 1.0;

/// The active controller put where navigation has the map before anything
/// observes it, where that is a jump away or the controller is not placed
/// yet: what the effects turn set the map center to is what the turn's
/// demand is made from, and a search asks for the pages of where it went and
/// not of where the view was. A move within the jump distance is left to the
/// tick's damping, so what is observed is the view being drawn and not the
/// control's target. A tracker takes no navigation.
pub fn snap_step() {
    match active() {
        Active::Orbit => {
            crate::control::camera::orbit::snap_if_discontinuous();
        }
        Active::Free => {
            crate::control::camera::free::navigate();
        }
        Active::Tracked => {}
    }
}

/// Which controller makes the pose now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Active {
    Orbit,
    Free,
    Tracked,
}

/// A tracker holding the pose: the tracker's space placed in the world as
/// it was when the tracker took over, and where the tracker has the camera
/// in it now. Rotations are the camera's axes to the frame's, rotation
/// alone.
struct Tracked {
    world_turn: Mat4,
    world_at: V3,
    turn: Mat4,
    at: V3,
}

struct Controllers {
    base: CameraMode,
    tracked: Option<Tracked>,
    /// How far the view looks, for the pose it was measured at.
    look: Option<(ObserverPose, f64)>,
}

static CONTROLLERS: Global<Controllers> = Global::new();

fn controllers() -> &'static mut Controllers {
    CONTROLLERS.get(|| Controllers {
        base: CameraMode::orbit,
        tracked: None,
        look: None,
    })
}

pub fn active() -> Active {
    let c = controllers();
    match (&c.tracked, c.base) {
        (Some(_), _) => Active::Tracked,
        (None, CameraMode::orbit) => Active::Orbit,
        (None, CameraMode::free) => Active::Free,
    }
}

/// The pose the active controller makes.
pub fn pose() -> ObserverPose {
    let c = controllers();
    match (&c.tracked, c.base) {
        (Some(t), _) => {
            let mut camera_to_world = mat4();
            mul3(&mut camera_to_world, &t.world_turn, &t.turn);
            let moved = apply3(&t.world_turn, t.at);
            ObserverPose {
                eye: std::array::from_fn(|k| t.world_at[k] + moved[k]),
                rotation: transpose3(&camera_to_world),
            }
        }
        (None, CameraMode::orbit) => orbit_pose(orbit()),
        (None, CameraMode::free) => free_pose(free()),
    }
}

/// How far the view looks: from the eye to what it looks at along its
/// forward ray — the orbit's distance to its pivot, and for any other
/// controller what its look falls on (free.rs `look_point`), measured once a
/// pose. What reads it takes the view's reach, whichever controller made it.
pub fn look_distance() -> f64 {
    if active() == Active::Orbit {
        return orbit().dist;
    }
    let now = view().pose;
    let c = controllers();
    if let Some((_, d)) = c.look.filter(|(at, _)| *at == now) {
        return d;
    }
    let r = &now.rotation;
    let forward = [-r[2], -r[6], -r[10]];
    let at = look_point(now.eye, forward, camera_store().view_state.dist);
    let d = (0..3).map(|k| (at[k] - now.eye[k]).powi(2)).sum::<f64>().sqrt();
    c.look = Some((now, d));
    d
}

/// Every controller's positions moved with the origin, `by` the origin's
/// own move: the base under a tracker too, which it comes back to.
pub fn rebase(by: V3) {
    let o = orbit();
    o.pivot = std::array::from_fn(|k| o.pivot[k] - by[k]);
    crate::control::camera::free::rebase(by);
    if let Some(t) = &mut controllers().tracked {
        t.world_at = std::array::from_fn(|k| t.world_at[k] - by[k]);
    }
}

/// The controllers' turn, first in the interaction phase: a change of the
/// settings' camera mode taken, the view where it was.
pub fn controller_step(ctx: &mut TickContext) {
    let want = settings_store().camera_mode;
    let c = controllers();
    if c.base == want {
        return;
    }
    match want {
        CameraMode::free => from_orbit(orbit()),
        CameraMode::orbit => {
            let o = to_orbit(free(), orbit().dist);
            let mut center = pxl_spec::space::pvf3::pvf3::Pvf3::default();
            local_world_to_canonical(&mut center, &o.pivot, view().origin);
            // The orbit stands about the map center, so it is where the
            // orbit's pivot is; and its controls are where it stands, so
            // nothing eases away.
            map_store().map_center = Some(center);
            camera_store().commit_view(crate::control::camera::store::ViewState {
                dist: o.dist,
                azimuth: o.azimuth,
                polar: o.polar,
            });
            *orbit() = o;
        }
    }
    c.base = want;
    view_compose();
    ctx.invalidate();
}

/// A tracked pose: the camera at `at` in the tracker's space, turned by the
/// unit quaternion `q` (x, y, z, w) from the camera's axes to the tracker's.
/// The first of a run places the tracker's space so the camera stays where
/// it is.
pub fn tracked_pose(at: V3, q: [f64; 4]) {
    let turn = quaternion(q);
    let c = controllers();
    if c.tracked.is_none() {
        let now = view().pose;
        // The camera's axes to the world's is the pose's turn reversed; the
        // tracker's space goes in so that where the tracker has the camera
        // now is where the camera is.
        let camera_to_world = transpose3(&now.rotation);
        let mut world_turn = mat4();
        mul3(&mut world_turn, &camera_to_world, &transpose3(&turn));
        let moved = apply3(&world_turn, at);
        c.tracked = Some(Tracked {
            world_turn,
            world_at: std::array::from_fn(|k| now.eye[k] - moved[k]),
            turn,
            at,
        });
    } else if let Some(t) = &mut c.tracked {
        (t.turn, t.at) = (turn, at);
    }
    view_compose();
    request_world_tick();
}

/// The tracker let go: the base controller is the pose again, as it was.
pub fn tracked_end() {
    if controllers().tracked.take().is_some() {
        view_compose();
        request_world_tick();
    }
}

/// The rotation a unit quaternion (x, y, z, w) makes, as the rotation
/// block of a column-major matrix.
fn quaternion([x, y, z, w]: [f64; 4]) -> Mat4 {
    let n = (x * x + y * y + z * z + w * w).sqrt();
    let (x, y, z, w) = (x / n, y / n, z / n, w / n);
    let mut m = mat4();
    m[0] = 1.0 - 2.0 * (y * y + z * z);
    m[1] = 2.0 * (x * y + z * w);
    m[2] = 2.0 * (x * z - y * w);
    m[4] = 2.0 * (x * y - z * w);
    m[5] = 1.0 - 2.0 * (x * x + z * z);
    m[6] = 2.0 * (y * z + x * w);
    m[8] = 2.0 * (x * z + y * w);
    m[9] = 2.0 * (y * z - x * w);
    m[10] = 1.0 - 2.0 * (x * x + y * y);
    m
}

/// The rotation blocks of two column-major matrices multiplied, into `out`.
fn mul3(out: &mut Mat4, a: &Mat4, b: &Mat4) {
    let mut m = mat4();
    for c in 0..3 {
        for r in 0..3 {
            m[c * 4 + r] = (0..3).map(|k| a[k * 4 + r] * b[c * 4 + k]).sum();
        }
    }
    *out = m;
}

fn transpose3(m: &Mat4) -> Mat4 {
    let mut t = mat4();
    for c in 0..3 {
        for r in 0..3 {
            t[c * 4 + r] = m[r * 4 + c];
        }
    }
    t
}

fn apply3(m: &Mat4, v: V3) -> V3 {
    std::array::from_fn(|r| (0..3).map(|k| m[k * 4 + r] * v[k]).sum())
}

#[cfg(test)]
#[path = "controller.test.rs"]
mod test;
