// pxl-core/domain/control/camera/controller.rs

//! Which controller moves the camera (camera.rs), where its eye stands for
//! the camera as drawn, and what the camera is told to do. The orbit's eye
//! is the camera's distance behind its pivot along its facing, the two
//! carried together over the ground on a flight. The first-person camera's
//! eye is over the pivot, where a body standing there looks from. So a
//! change of controller moves the eye about a pivot that stays.
//!
//! The camera does what it is told and refuses nothing: who may tell it
//! what, and when, is said above it (domain/control/navigation.rs).

use crate::spec::step::InvalidationReason;
use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::V3;

use crate::domain::control::camera::first_person;
use crate::domain::control::camera::motion::{Move, end, motion};
use crate::domain::control::camera::orbit;
use crate::domain::control::camera::pivot::{self, pan_to, stood, stood_at};
use crate::domain::control::camera::{Camera, camera, facing_rot};
use crate::domain::control::param::{DEFAULT_AZIMUTH, DEFAULT_DIST, DEFAULT_POLAR};
use crate::domain::eye::EyePose;
use crate::domain::query::floor_of;
use crate::domain::query::leaf_drawn;
use crate::domain::setting::CameraMode;
use crate::domain::view::frame::coord::{to_world, world_to_canonical};
use crate::domain::view::frame::screen::units_per_px_at;
use crate::sys::sched::invalidate;
use crate::sys::sched::tick::TickContext;
use pxl_global::Global;

// ---- what is read of the camera ----

static MODE: Global<CameraMode> = Global::new();

/// Which controller moves the camera.
pub(crate) fn mode() -> CameraMode {
    *MODE.get(|| CameraMode::Orbit)
}

/// The pose the camera is drawn at. None before the camera is put.
pub(crate) fn pose() -> Option<EyePose> {
    let at = drawn((*camera())?);
    Some(EyePose {
        eye: match mode() {
            CameraMode::Orbit => orbit::eye(&at),
            CameraMode::FirstPerson => first_person::eye(&at),
        },
        rot: facing_rot(at.polar, at.azimuth),
    })
}

/// `cam` as the controller that moves it draws it.
fn drawn(cam: Camera) -> Camera {
    match mode() {
        CameraMode::Orbit => orbit::drawn(&cam),
        CameraMode::FirstPerson => cam,
    }
}

/// The facing the camera is drawn at: its polar angle, radians from straight
/// down, and its azimuth, radians from north toward east. None before the
/// camera is put.
pub(crate) fn facing() -> Option<(f64, f64)> {
    let cam = drawn((*camera())?);
    Some((cam.polar, cam.azimuth))
}

/// The distance the view is scaled by, the orbit's (orbit.rs `view_dist`): 0
/// where the camera has no zoom, in first person, where a scale has nothing
/// to say (ui.rs `m_per_px`).
pub(crate) fn dist() -> f64 {
    match (mode(), *camera()) {
        (CameraMode::Orbit, Some(cam)) => orbit::view_dist(&cam),
        (CameraMode::Orbit, None) => DEFAULT_DIST,
        (CameraMode::FirstPerson, _) => 0.0,
    }
}

/// The camera's pivot where it stands, on the ground. A flight carries the
/// camera over it (orbit.rs `at_height`). None before the camera is put.
pub(crate) fn pivot() -> Option<Q3> {
    Some(world_to_canonical(&(*camera())?.pivot))
}

/// Where the pivot is going to: the end of the camera's move, or where it
/// is drawn on no move. None before the camera is put.
pub(crate) fn pivot_to() -> Option<Q3> {
    let cam = camera().as_ref()?;
    Some(world_to_canonical(&end(cam).to))
}

/// The top of the floor the pivot stands on where it is going. None before
/// the camera is put.
pub(crate) fn floor_to() -> Option<f64> {
    Some(floor_of(&stood(&pivot_to()?)))
}

/// Whether the camera is on a move.
pub(crate) fn moving() -> bool {
    motion().is_some()
}

/// Drops what the camera keeps of the ground where a change touched it: the
/// floor its pivot stands on, and the orbit's pitch and arm.
pub(crate) fn forget_touched() {
    pivot::forget_touched();
    orbit::forget_touched();
}

// ---- what the camera is told ----

/// Has the camera end as `change` makes of where it ends now (motion.rs
/// `end`). A camera never put has nothing to move.
fn end_as(change: impl FnOnce(Move) -> Move) {
    let Some(cam) = camera().as_ref() else {
        return;
    };
    let was = end(cam);
    let to = change(was).in_range();
    if to != was {
        *motion() = Some(to);
        invalidate(InvalidationReason::View);
    }
}

/// Puts a camera never put about `at`, at the default distance and facing.
fn put(at: V3) {
    *camera() = Some(Camera {
        pivot: at,
        dist: DEFAULT_DIST,
        polar: DEFAULT_POLAR,
        azimuth: DEFAULT_AZIMUTH,
    });
    invalidate(InvalidationReason::View);
}

/// Warps `cam` to `at`, where it is not there: its pivot is there at once,
/// with nothing between, and a move it is on ends there, its flight over, so
/// its distance and its facing are eased on.
fn warp(cam: &mut Camera, at: V3) {
    if cam.pivot == at {
        return;
    }
    cam.pivot = at;
    if let Some(m) = motion() {
        (m.to, m.speed, m.height) = (at, 0.0, 0.0);
    }
    invalidate(InvalidationReason::View);
}

/// Whether the camera goes to `at` on a move, or warps there: the orbit
/// always goes, and first person, whose eye never leaves its body, only to
/// a place the eye draws as it is, with the leaves' cells.
fn goes_to(at: V3) -> bool {
    match mode() {
        CameraMode::Orbit => true,
        CameraMode::FirstPerson => leaf_drawn(at),
    }
}

/// Has the camera go to the point `to`: its move ends there, the orbit
/// flown (orbit.rs `step`) and first person eased, or it warps (`goes_to`).
/// A camera never put is put there.
pub(crate) fn go(to: Q3) {
    let at = to_world(&to);
    match camera() {
        Some(_) if goes_to(at) => end_as(|m| Move { to: at, ..m }),
        Some(cam) => warp(cam, at),
        None => put(at),
    }
}

/// Warps the camera to the point `to`. A camera never put is put there.
pub(crate) fn warp_to(to: Q3) {
    let at = to_world(&to);
    match camera() {
        Some(cam) => warp(cam, at),
        None => put(at),
    }
}

/// The camera stays where its pivot is: a move under way ends there, so
/// the orbit comes down in place from a flight. Its distance and its facing
/// are eased on to where they were asked.
pub(crate) fn stay() {
    if let (Some(m), Some(cam)) = (motion().as_mut(), camera().as_ref()) {
        m.to = cam.pivot;
        invalidate(InvalidationReason::View);
    }
}

/// A view farther out than the default comes in to it, eased, and one
/// brought nearer stays.
pub(crate) fn come_in() {
    end_as(|m| Move {
        dist: m.dist.min(DEFAULT_DIST),
        ..m
    });
}

/// Turns the view to `azimuth`, eased.
pub(crate) fn azimuth_to(azimuth: f64) {
    end_as(|m| Move { azimuth, ..m });
}

/// Rotates the camera by `azimuth` and tilts it by `polar`, in radians: the
/// facing is eased there, and what is asked adds up.
pub(crate) fn face_by(azimuth: f64, polar: f64) {
    end_as(|m| Move {
        azimuth: m.azimuth + azimuth,
        polar: m.polar + polar,
        ..m
    });
}

/// Zooms by `scale`, which divides the camera's distance: the distance is
/// eased there, and what is asked adds up. First person has no zoom, so it
/// ignores it.
pub(crate) fn zoom(scale: f64) {
    if mode() == CameraMode::Orbit {
        end_as(|m| Move {
            dist: m.dist / scale,
            ..m
        });
    }
}

/// The distance a pan is scaled by, the one the view is drawn at (`dist`).
/// First person pans as the orbit does at `DEFAULT_DIST`, since it has no
/// zoom distance of its own.
fn across() -> f64 {
    match mode() {
        CameraMode::Orbit => dist(),
        CameraMode::FirstPerson => DEFAULT_DIST,
    }
}

/// Pans by `dx`, `dy` pixels, so the ground follows the pointer.
pub(crate) fn pan(dx: f64, dy: f64) {
    let upp = units_per_px_at(across());
    pan_by(-dx * upp, dy * upp);
}

/// Pans by `cx` and `cy` times the distance to the pivot, across and up the
/// screen, the same apparent length at every zoom.
pub(crate) fn pan_across(cx: f64, cy: f64) {
    let d = across();
    pan_by(cx * d, cy * d);
}

/// Pans the pivot at once, across the ground from where it stands
/// (pivot.rs `pan_to`). The pivot's height follows on the move, so a step up
/// or down is eased. A camera never put has nothing to pan.
fn pan_by(lx: f64, ly: f64) {
    let Some(cam) = camera().as_mut() else {
        return;
    };
    let was = end(cam);
    let to = pan_to(&cam.pivot, lx, ly, cam.azimuth);
    cam.pivot = [to[0], to[1], cam.pivot[2]];
    *motion() = Some(Move { to, ..was });
    invalidate(InvalidationReason::View);
}

/// Has the controller `want` move the camera. The camera stays as it is,
/// and its eye is where the new controller draws it; a move goes on, with
/// no flight, which is the orbit's, left in it.
pub(crate) fn set_mode(want: CameraMode) {
    if mode() != want {
        MODE.set(want);
        orbit::level();
        if let Some(m) = motion() {
            (m.speed, m.height) = (0.0, 0.0);
        }
        invalidate(InvalidationReason::View);
    }
}

// ---- the camera's step ----

pub(crate) fn camera_step(ctx: &mut TickContext) {
    if step(ctx.delta) {
        invalidate(InvalidationReason::View);
    }
}

/// Moves the camera over `delta` seconds and returns whether it moved. On a
/// move, the distance and the facing are eased, the orbit flies to where its
/// pivot is to go, its whole camera carried, and first person's pivot is
/// eased; the move ends where the camera can no longer be told from its
/// end. The pivot stands on its floor (pivot.rs `stood`): a move's end is
/// kept on it, and a camera on no move whose floor changed moves onto it.
/// The pitch the orbit climbs to is eased beside the move. The frame a move
/// ends counts as moved, so whoever waits on it takes the frame after.
pub(crate) fn step(delta: f64) -> bool {
    let mode = mode();
    let Some(cam) = camera().as_mut() else {
        return false;
    };
    let was = end(cam);
    let to = Move {
        to: stood_at(&was.to),
        ..was
    };
    if to != was {
        *motion() = Some(to);
    }
    let on = moving();
    let moves = motion().as_mut().is_some_and(|m| match mode {
        CameraMode::Orbit => orbit::step(cam, m, delta),
        CameraMode::FirstPerson => first_person::step(cam, m, delta),
    });
    if !moves {
        *motion() = None;
    }
    let climbs = mode == CameraMode::Orbit && orbit::climb_step(cam, delta);
    on || climbs
}

#[cfg(test)]
#[path = "controller.test.rs"]
mod test;
