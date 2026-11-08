// pxl-core/domain/control/camera/controller.rs

//! Which controller moves the camera (camera.rs), and the pivot it turns
//! about. The orbit's pivot is a point `dist` ahead of the eye. The
//! first-person camera's pivot is the eye itself. The pivot is derived each
//! time it is asked for and never kept. A tracker (`tracked_pose`) overrides
//! either controller while it holds the pose, because a tracked turn can
//! roll and neither controller has a roll.

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};

use crate::domain::control::camera::first_person;
use crate::domain::control::camera::orbit;
use crate::domain::control::camera::orbit::orbit;
use crate::domain::control::camera::state::{camera_state, Intent, IntentPatch};
use crate::domain::control::camera::{camera, eye_world, facing_rot};
use crate::domain::control::param::{DEFAULT_AZIMUTH, DEFAULT_DIST};
use crate::domain::control::support::{is_unknown, pan_to, unknown_z};
use crate::domain::setting::store::{settings_store, CameraMode};
use crate::domain::view::frame::coord::{world_to_canonical, world_to_canonical_nearest};
use crate::domain::view::frame::pose::ObserverPose;
use crate::domain::view::frame::screen::units_per_px_at;
use crate::sys::sched::edge::request_world_tick;
use crate::sys::sched::tick::TickContext;
use xpute_core::global::Global;

/// The distance from the eye to the pivot. It is zero in first person and
/// while tracked.
pub fn dist() -> f64 {
    match active() {
        Active::Orbit => orbit().dist,
        Active::FirstPerson | Active::Tracked => 0.0,
    }
}

pub fn pivot_world() -> Option<V3> {
    match active() {
        Active::Orbit => orbit::pivot_world(),
        Active::FirstPerson => eye_world(),
        Active::Tracked => Some(pose()?.eye),
    }
}

pub fn pivot() -> Option<Q3> {
    let at = pivot_world()?;
    let c = world_to_canonical(&at);
    Some(c)
}

/// How the camera moves to a new pivot. A place found elsewhere (a search,
/// a recenter, the start, a change of person) is snapped to. A place on the
/// screen or a followed location fix is eased to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Snap,
    Smooth,
}

/// Moves the pivot to `to`. While a tracker holds the pose, this does
/// nothing, and the controller under the tracker does not move in its place.
pub fn put_pivot(to: Q3, motion: Motion) {
    let snap = motion == Motion::Snap;
    match active() {
        Active::Tracked => return,
        Active::Orbit => orbit::put(to, snap),
        Active::FirstPerson => first_person::put(to, snap),
    }
    request_world_tick();
}

/// Where the pivot is going to: the place last put, which a smooth move
/// eases toward, or else where the pivot stands. None while tracked.
pub fn pivot_to() -> Option<Q3> {
    match active() {
        Active::Orbit => orbit::pivot_to(),
        Active::FirstPerson => first_person::pivot_to(),
        Active::Tracked => None,
    }
}

/// The distance a pan is scaled by. First person pans as the orbit does at
/// `DEFAULT_DIST`, since it has no zoom distance of its own.
fn across() -> f64 {
    match active() {
        Active::Orbit => camera_state().intent.dist,
        Active::FirstPerson | Active::Tracked => DEFAULT_DIST,
    }
}

/// Pans by a drag of `dx`, `dy` pixels, so the ground follows the pointer.
pub fn pan(dx: f64, dy: f64) {
    let upp = units_per_px_at(across());
    pan_by(-dx * upp, dy * upp);
}

/// Pans by `cx` and `cy` times the distance to the pivot, across and up the
/// screen, so a held key keeps the same apparent speed at every zoom.
pub fn pan_across(cx: f64, cy: f64) {
    let d = across();
    pan_by(cx * d, cy * d);
}

fn pan_by(lx: f64, ly: f64) {
    let Some(from) = pivot_to() else { return };
    let eye = eye_world().map_or(from, |e| world_to_canonical(&e));
    put_pivot(
        pan_to(&from, lx, ly, camera().azimuth, &eye),
        Motion::Smooth,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Active {
    Orbit,
    FirstPerson,
    Tracked,
}

/// Where the tracker's space lies in the world, fixed when the tracker took
/// over, and where the camera is in that space now.
struct TrackerSpace {
    world_turn: Mat4,
    world_at: V3,
    turn: Mat4,
    at: V3,
}

struct Controllers {
    base: CameraMode,
    tracker: Option<TrackerSpace>,
}

static CONTROLLERS: Global<Controllers> = Global::new();

fn controllers() -> &'static mut Controllers {
    CONTROLLERS.get(|| Controllers {
        base: CameraMode::orbit,
        tracker: None,
    })
}

pub fn active() -> Active {
    let c = controllers();
    match (&c.tracker, c.base) {
        (Some(_), _) => Active::Tracked,
        (None, CameraMode::orbit) => Active::Orbit,
        (None, CameraMode::first_person) => Active::FirstPerson,
    }
}

/// The pose the camera is drawn from. None before the camera has an eye.
pub fn pose() -> Option<ObserverPose> {
    camera().eye?;
    let c = controllers();
    match (&c.tracker, c.base) {
        (Some(t), _) => {
            let (mut camera_to_world, mut rot, mut moved) = (mat4::new(), mat4::new(), [0.0; 3]);
            mat4::mul(&mut camera_to_world, &t.world_turn, &t.turn);
            mat4::transform_point(&mut moved, &t.world_turn, t.at[0], t.at[1], t.at[2]);
            Some(ObserverPose {
                eye: v3::add(&t.world_at, &moved),
                rot: *mat4::transpose(&mut rot, &camera_to_world),
            })
        }
        (None, _) => {
            let cam = camera();
            Some(ObserverPose {
                eye: eye_world()?,
                rot: facing_rot(cam.polar, cam.azimuth),
            })
        }
    }
}

/// Returns the intent the controller took, which a gesture continues from
/// (gesture.rs `Anchor::settle`).
pub fn set_intent(partial: IntentPatch) -> Intent {
    match active() {
        Active::Orbit => orbit::ask(partial),
        Active::FirstPerson | Active::Tracked => {
            let c = camera_state();
            c.commit_intent(c.with_patch(partial));
            c.intent
        }
    }
}

/// Turns the camera to north and keeps the tilt. That is what "align north"
/// means both for the compass and at the end of follow mode.
pub fn reset_azimuth() {
    set_intent(IntentPatch {
        azimuth: Some(DEFAULT_AZIMUTH),
        ..Default::default()
    });
}

pub fn reset_dist() {
    set_intent(IntentPatch {
        dist: Some(DEFAULT_DIST),
        ..Default::default()
    });
}

/// Used when going to a place. A view farther out than the default comes
/// back in to it, and a view the user brought nearer stays.
pub fn come_in_to_default() {
    let default = DEFAULT_DIST;
    if camera_state().intent.dist > default {
        reset_dist();
    }
}

/// A wheel turn of `delta_y` CSS pixels. First person has no zoom, so it
/// ignores the wheel.
pub fn wheel(delta_y: f64) {
    if active() == Active::Orbit {
        orbit::wheel(delta_y);
    }
}

pub fn pinch_begin(span: f64) {
    orbit::pinch_begin(span);
}

pub fn pinch(span: f64) {
    if active() == Active::Orbit {
        orbit::pinch(span);
    }
}

pub fn controller_step(ctx: &mut TickContext) {
    if switch() {
        ctx.invalidate();
    }
}

pub fn camera_step(ctx: &mut TickContext) {
    let moving = match active() {
        Active::Orbit => orbit::turn(ctx.delta) == Some(true),
        Active::FirstPerson => first_person::turn(ctx.delta),
        Active::Tracked => false,
    };
    if moving {
        ctx.invalidate();
    }
}

fn switch() -> bool {
    if !take_mode(settings_store().camera_mode) {
        return false;
    }
    request_world_tick();
    true
}

/// Switches to the controller `want` and returns whether it switched. The
/// new controller's camera is put at the current pivot at once, so nothing
/// eases away. While a tracker holds the pose, the switch waits until the
/// tracker lets go.
fn take_mode(want: CameraMode) -> bool {
    let c = controllers();
    if c.base == want || c.tracker.is_some() {
        return false;
    }
    // The pivot is taken to the nearest canonical point. Any other rounding
    // would walk the character away over a few switches back and forth. A
    // height not yet known stays unknown.
    let unknown = pivot_to().is_some_and(|t| is_unknown(&t));
    let at = pivot_world().map(|local| {
        let c = world_to_canonical_nearest(&local);
        if unknown {
            unknown_z(&c)
        } else {
            c
        }
    });
    c.base = want;
    let cam = camera();
    if want == CameraMode::orbit {
        camera_state().commit_intent(Intent {
            dist: orbit().dist,
            azimuth: cam.azimuth,
            polar: cam.polar,
        });
    }
    if let Some(at) = at {
        match want {
            CameraMode::orbit => orbit::put(at, true),
            CameraMode::first_person => first_person::put(at, true),
        }
    }
    true
}

/// A tracker's pose: the camera at `at` in the tracker's space, and the
/// quaternion `q` (x, y, z, w) that turns the camera's axes to the
/// tracker's. The first pose of a run places the tracker's space in the
/// world so that the camera stays where it is. Before the camera has an
/// eye, there is nowhere to place it, and the pose is dropped.
pub fn tracked_pose(at: V3, q: [f64; 4]) {
    let mut turn = mat4::new();
    mat4::rot_quat(&mut turn, q);
    let c = controllers();
    if c.tracker.is_none() {
        let Some(now) = pose() else { return };
        let (mut camera_to_world, mut turn_back, mut world_turn, mut off) =
            (mat4::new(), mat4::new(), mat4::new(), [0.0; 3]);
        mat4::transpose(&mut camera_to_world, &now.rot);
        mat4::mul(
            &mut world_turn,
            &camera_to_world,
            mat4::transpose(&mut turn_back, &turn),
        );
        mat4::transform_point(&mut off, &world_turn, at[0], at[1], at[2]);
        c.tracker = Some(TrackerSpace {
            world_turn,
            world_at: v3::sub(&now.eye, &off),
            turn,
            at,
        });
    } else if let Some(t) = &mut c.tracker {
        (t.turn, t.at) = (turn, at);
    }
    request_world_tick();
}

pub fn tracked_end() {
    if controllers().tracker.take().is_some() {
        request_world_tick();
    }
}

#[cfg(test)]
#[path = "controller.test.rs"]
mod test;
