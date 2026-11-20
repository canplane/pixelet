// pxl-core/domain/control/camera/orbit.rs

//! The orbit camera. The eye moves on a sphere about the pivot, and the
//! sphere's radius is the zoom. A zoom, a rotation and a tilt set how the eye
//! stands to the pivot, its distance and its facing, and are eased. What
//! takes the pivot carries the whole camera, with that unchanged: a pan at
//! once, and a flight to where it is told to go
//! (domain/smooth.rs `fly`). `step` takes the camera along a
//! move (motion.rs `Move`) by both. Nothing of the ground enters either.
//!
//! The ground bounds the camera as it is drawn and nowhere else, where the
//! eye is and reads the world as it draws it, by the obstacle setting:
//!
//! - `through`: as it moves.
//! - `climb`: the pitch the orbit is drawn at rises until the eye stands
//!   over the floor under it, by a cell, or at the nearest by the near
//!   plane's distance (`climb_polar`). It rises fast and relaxes
//!   slowly. Where no pitch clears the eye, as under a tunnel's ceiling, the
//!   eye is drawn as `stop` draws it.
//! - `stop`: the eye is drawn no farther from the pivot than the first thing
//!   on the line from the pivot toward it (`at_arm`), as a spring arm is.
//!   The camera moves freely under it, so the eye is back out as soon as
//!   the line is open.
//!
//! Within `NEAREST_DIST` of the pivot and above its level, the climb draws
//! the eye where the camera has it, through whatever is there: the eye's rays do not meet
//! the cell it stands in (pxl-engine `cast::walk`), so nothing there hides
//! the view. Below the level it keeps the eye over the floor under it as
//! anywhere. The arm is no shorter than `MIN_DIST`.

use crate::domain::control::camera::motion::{Move, motion};
use crate::domain::control::camera::{Camera, ease_facing, eases_toward, facing_dir};
use crate::domain::control::param::CLIMB_PASS_CNT;
use crate::domain::control::param::{CLIMB_RELAX_DAMP, CLIMB_RISE_DAMP, DIST_DAMP, FLIGHT};
use crate::domain::control::param::{EYE_CLEARANCE_U, MIN_DIST, NEAREST_DIST};
use crate::domain::param::POS_DAMP;
use crate::domain::query::change::Memo;
use crate::domain::query::{Ray, SizeBy, World};
use crate::domain::setting::{OrbitObstacleMode, settings};
use crate::domain::smooth::{View, fly, smooth_vec};
use crate::domain::view::frame::coord::world_to_canonical;
use crate::domain::view::param::NEAR;
use pxl_global::Global;
use pxl_spec::space::coord::v3::{self, V3};

fn eye_off(dist: f64, polar: f64, azimuth: f64) -> V3 {
    facing_dir(polar, azimuth).map(|v| -v * dist)
}

/// Where the eye of `cam` stands: its distance behind the pivot, along its
/// facing.
pub(crate) fn eye(cam: &Camera) -> V3 {
    v3::add(&cam.pivot, &eye_off(cam.dist, cam.polar, cam.azimuth))
}

/// How high the flight the camera is on carries it (motion.rs `Move`), 0 on
/// no move.
fn height() -> f64 {
    motion().map_or(0.0, |m| m.height)
}

/// How far the view is scaled by: the eye's distance from its pivot as it is
/// drawn, and how high a flight carries the camera. A scale and a pan
/// measure the ground on the screen with it; no gesture sets it. It is
/// the eye's distance to the ground looking straight down, and short of it
/// on a flight the more the camera is tilted.
pub(crate) fn view_dist(cam: &Camera) -> f64 {
    drawn(cam).dist + height()
}

/// `cam` where its flight has it: as high over where it stands as the flight
/// carries it.
fn at_height(cam: &Camera) -> Camera {
    lifted(cam, height())
}

/// `cam` carried `height` over where it stands.
fn lifted(cam: &Camera, height: f64) -> Camera {
    let [x, y, z] = cam.pivot;
    Camera {
        pivot: [x, y, z + height],
        ..*cam
    }
}

/// Moves the orbit `cam` along the move `mv` over `delta` seconds and
/// returns whether it is still moving. The distance and the facing are eased
/// to the move's. Where the pivot is to go is a flight of the whole camera,
/// its eye and its pivot together: across the ground along
/// the flight's way (domain/smooth.rs `fly`), level to a place beside it and
/// higher over a longer way, the view as wide as the camera's distance and
/// the flight's height together, and down again where it ends. The pivot's
/// own height, on its floor, is eased.
pub(crate) fn step(cam: &mut Camera, mv: &mut Move, delta: f64) -> bool {
    let to = mv.to;
    let now = lifted(cam, mv.height);
    let eye_to = v3::add(&to, &eye_off(mv.dist, mv.polar, mv.azimuth));
    let moving = eases_toward(&now, eye(&now), eye_to, mv.polar, mv.azimuth);
    if !moving {
        *cam = Camera {
            pivot: to,
            dist: mv.dist,
            polar: mv.polar,
            azimuth: mv.azimuth,
        };
        return false;
    }
    ease_facing(cam, mv.polar, mv.azimuth, delta);
    [cam.dist] = smooth_vec([cam.dist], [mv.dist], DIST_DAMP, delta);
    let from = View {
        at: [cam.pivot[0], cam.pivot[1]],
        w: cam.dist + mv.height,
    };
    let to_view = View {
        at: [to[0], to[1]],
        w: cam.dist,
    };
    let v = fly(&mut mv.speed, from, to_view, &FLIGHT, delta);
    mv.height = (v.w - cam.dist).max(0.0);
    let [z] = smooth_vec([cam.pivot[2]], [to[2]], POS_DAMP, delta);
    cam.pivot = [v.at[0], v.at[1], z];
    true
}

/// The pitch the climb holds the orbit at, none where the ground asks
/// nothing and the orbit is drawn at the camera's own pitch.
static CLIMB: Global<Option<f64>> = Global::new();

fn climb() -> &'static mut Option<f64> {
    CLIMB.get(|| None)
}

/// The pitch `cam` is drawn at: its own, or the climb's where that is
/// steeper. A tilt therefore shows as the camera's own pitch eases wherever
/// the climb holds nothing, and changes nothing drawn under what it holds.
fn pitch(cam: &Camera) -> f64 {
    climb().map_or(cam.polar, |held| held.min(cam.polar))
}

/// Eases the pitch `cam` is drawn at toward the one the ground under its eye
/// asks under `climb` (`climb_polar`), over `delta` seconds, and returns
/// whether it is still moving. It rises fast and relaxes slowly.
pub(crate) fn climb_step(cam: &Camera, delta: f64) -> bool {
    let cam = &at_height(cam);
    let to = match settings().orbit_obstacle {
        OrbitObstacleMode::Climb => climb_polar(cam),
        _ => cam.polar,
    };
    let was = pitch(cam);
    let rate = match to < was {
        true => CLIMB_RISE_DAMP,
        false => CLIMB_RELAX_DAMP,
    };
    let [eased] = smooth_vec([was], [to], rate, delta);
    let at = |polar: f64| Camera { polar, ..*cam };
    let (now, end) = (at(eased), at(to));
    let moving = eases_toward(&now, eye(&now), eye(&end), end.polar, end.azimuth);
    let drawn = match moving {
        true => eased,
        false => to,
    };
    *climb() = (drawn < cam.polar).then_some(drawn);
    moving
}

/// The climb holds nothing: another controller draws the camera.
pub(crate) fn level() {
    *climb() = None;
}

/// `cam` as it is drawn: where its flight has it, at the pitch the climb
/// holds it at, and its eye no farther from the pivot than its arm reaches
/// (`at_arm`).
pub(crate) fn drawn(cam: &Camera) -> Camera {
    let cam = at_height(cam);
    at_arm(&Camera {
        polar: pitch(&cam),
        ..cam
    })
}

/// The floor under the eye of `at` (`World::floor_under`), None where nothing
/// is known under the eye.
fn floor_under_eye(at: &Camera, world: &World) -> Option<f64> {
    world
        .floor_under(&world_to_canonical(&eye(at)), SizeBy::Eye)
        .val()
        .flatten()
}

/// The height above the pivot that puts the eye over the floor under it, as
/// `floor_under_eye` reads that floor: one cell over it (`EYE_CLEARANCE_U`),
/// and at the nearest the near plane's distance, so a tilt below the pivot's
/// level ends at the floor. Ground and placed objects count alike, at any
/// distance. None where nothing is known under the eye.
fn climb_rise(at: &Camera, world: &World) -> Option<f64> {
    let clear = match at_nearest(at) {
        true => NEAR,
        false => EYE_CLEARANCE_U,
    };
    Some(floor_under_eye(at, world)? + clear - at.pivot[2])
}

/// Whether `at` is at the nearest (`NEAREST_DIST`).
fn at_nearest(at: &Camera) -> bool {
    at.dist <= NEAREST_DIST
}

fn in_solid(at: &Camera, world: &World) -> bool {
    floor_under_eye(at, world).is_some_and(|top| top > eye(at)[2])
}

/// `cam` as the ground lets it be drawn, its eye no farther from the pivot
/// than its arm reaches (`arm_of`). The answer is kept as `climb_polar`'s is
/// (`ARM`), by the obstacle setting too.
fn at_arm(cam: &Camera) -> Camera {
    let mode = settings().orbit_obstacle;
    let key = *cam;
    let dist = ARM
        .get(|| Memo::NONE)
        .of((key, mode), || arm_of(cam, mode, &World::now()));
    Camera { dist, ..*cam }
}

/// How far from its pivot the eye of `at` is drawn. Under `through` it is the
/// distance of `at`. Under `stop`, and under `climb` where no pitch clears
/// the eye (`at_climb`), as under a tunnel's ceiling, it ends before the
/// first thing on the line from the pivot toward the eye: short of it by the
/// near plane's distance, so the near plane never cuts into it, and no
/// nearer than `MIN_DIST`. The line is read as the eye reads the world
/// (`World::first_hit`): what a coarser tile stands in for is met too, and
/// decor never is.
fn arm_of(at: &Camera, mode: OrbitObstacleMode, world: &World) -> f64 {
    let reads_line = match mode {
        OrbitObstacleMode::Stop => true,
        OrbitObstacleMode::Climb => in_solid(&at_climb(*at, world), world),
        OrbitObstacleMode::Through => false,
    };
    if !reads_line {
        return at.dist;
    }
    let ray = Ray {
        origin: at.pivot,
        dir: eye_off(1.0, at.polar, at.azimuth),
        t_max: at.dist,
    };
    match world.first_hit(&ray).val().flatten() {
        Some(hit) => (hit.t - NEAR).clamp(MIN_DIST, at.dist.max(MIN_DIST)),
        None => at.dist,
    }
}

/// The arm last drawn, kept as `CLIMB_PITCH` is.
static ARM: Global<Memo<(Camera, OrbitObstacleMode), f64>> = Global::new();

/// The shallowest pitch at which the eye clears the terrain, solved as
/// `acos(h / dist)` for a rise `h` over the pivot.
///
/// The answer is kept while the camera stays the same and the ground it
/// read does not change (`CLIMB_PITCH`).
fn climb_polar(at: &Camera) -> f64 {
    let key = *at;
    CLIMB_PITCH
        .get(|| Memo::NONE)
        .of(key, || at_climb(*at, &World::now()).polar)
}

/// Drops the kept pitch and the kept arm where a change touched the rect
/// each read. It runs on every frame, so each change is seen exactly once
/// (`change::within`).
pub(crate) fn forget_touched() {
    CLIMB_PITCH.get(|| Memo::NONE).forget_touched();
    ARM.get(|| Memo::NONE).forget_touched();
}

/// The pitch of the last climb. It is kept while the camera stays the
/// same, the eye is given what its reads took of it and no change touches
/// what it read (`change::Memo`).
static CLIMB_PITCH: Global<Memo<Camera, f64>> = Global::new();

/// `at` as the climb draws it. At the nearest (`NEAREST_DIST`) and at or
/// above the pivot's level, it is drawn as it is. Otherwise its pitch is
/// steepened as the floor under the eye asks (`steepened`), and the
/// steepened pitch is kept even where it did not clear the floor. The camera's own
/// pitch is kept instead where the eye, at the distance of `at`, would be in
/// something solid even looking straight down, as under the ceiling of a
/// tunnel or a cave.
fn at_climb(at: Camera, world: &World) -> Camera {
    if at_nearest(&at) && at.polar <= core::f64::consts::FRAC_PI_2 {
        return at;
    }
    let climbed = steepened(at, world);
    match in_solid(&climbed, world) && in_solid(&Camera { polar: 0.0, ..at }, world) {
        true => at,
        false => climbed,
    }
}

/// `at` with its pitch steepened until the eye clears the floor under it
/// (`climb_rise`). The pitch only ever steepens. This holds past the
/// horizontal too, where the floor lies below the pivot and the rise is
/// zero or negative.
fn steepened(at: Camera, world: &World) -> Camera {
    let mut climbed = at;
    for _ in 0..CLIMB_PASS_CNT {
        let Some(rise) = climb_rise(&climbed, world) else {
            break;
        };
        let solved = libm::acos((rise / climbed.dist).clamp(-1.0, 1.0));
        if solved >= climbed.polar {
            break;
        }
        climbed.polar = solved;
    }
    climbed
}

#[cfg(test)]
#[path = "orbit.test.rs"]
mod test;
