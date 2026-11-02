// pxl-kernel/domain/control/camera/orbit/collision.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short.

use pxl_spec::space::coord::q3::Q3;
use pxl_spec::space::coord::v3::{self, V3};
use xpute_guest::global::Global;
use xpute_kit::math::scalar::wrap;

use crate::domain::control::camera::controller;
use crate::domain::control::camera::orbit::orbit_eye_off;
use crate::domain::control::camera::state::Intent;
use crate::domain::control::param::{CLIMB_PASSES, COLLISION_STEP_SAMPLES};
use crate::domain::control::param::{EYE_CLEARANCE_U, MIN_DIST};
use crate::domain::query::changes::Memo;
use crate::domain::query::{finest, Hit, Ray, World};
use crate::domain::view::frame::coord::world_to_canonical;
use crate::domain::view::param::NEAR;
use crate::domain::view::setting::store::{settings_store, OrbitObstacleMode};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

/// The pivot is part of the pose: the eye is `pivot + offset`, and a gate
/// that read only the offset could be walked through by panning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPose {
    pub pivot: V3,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

fn pose(view: Intent) -> Option<ViewPose> {
    Some(ViewPose {
        pivot: controller::pivot_world()?,
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// The top of the ground under `at`'s eye, the one floor the gate reads of
/// a pose: none where no tile is resident. Asked from the eye's own cell, so
/// a cave's floor counts where the eye is in the cave.
fn ground_under(at: &ViewPose, access: &World) -> Option<f64> {
    access.floor_under(&eye(at).1, LEAF_ZOOM).val().flatten()
}

/// Whether `at` is at the orbit's nearest, over the pivot's level, where the
/// eye turns about the pivot whatever is in the way: its rays do not meet the
/// cell it stands in (pxl-engine `cast::walk`).
fn nearest(at: &ViewPose) -> bool {
    at.dist <= MIN_DIST && at.polar <= core::f64::consts::FRAC_PI_2
}

/// Rise to the floor under the eye, a cell over it (`EYE_CLEARANCE_U`), as
/// `ground_under` reads it from the eye's own cell: ground and what is
/// placed alike, at any distance. None where nothing is known under it.
fn climb_rise(at: &ViewPose, access: &World) -> Option<f64> {
    Some(ground_under(at, access)? + EYE_CLEARANCE_U - at.pivot[2])
}

/// Positive inside the terrain, off the ground under its eye
/// (`ground_under`).
fn penetration(at: &ViewPose, ground: Option<f64>) -> Option<f64> {
    Some(ground? + EYE_CLEARANCE_U - eye(at).0[2])
}

/// The eye, local and canonical.
fn eye(at: &ViewPose) -> (V3, Q3) {
    let off = orbit_eye_off(at.dist, at.polar, at.azimuth);
    let world: V3 = v3::add(&at.pivot, &off);
    (world, world_to_canonical(&world))
}

/// A step may not end deeper in the ground than it started, but at the
/// nearest (`nearest`), which it always gets to.
fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    // Checked first so the setting skips the floor's walk.
    match settings_store().orbit_obstacle {
        OrbitObstacleMode::through => return *to,
        OrbitObstacleMode::climb => return climb_step(to, from, &World::now()),
        OrbitObstacleMode::stop if nearest(to) => return *to,
        OrbitObstacleMode::stop => {}
    }

    let access = World::now();
    let from_ground = ground_under(from, &access);
    let limit = penetration(from, from_ground).map_or(0.0, |start| start.max(0.0));

    // Each sample's ground asked once, whether it is deep and in something
    // solid both read off it, and the last's kept for the next.
    let (mut last, mut last_solid) = (*from, solid(from, from_ground));
    for i in 1..=COLLISION_STEP_SAMPLES {
        let at = between(from, to, i as f64 / COLLISION_STEP_SAMPLES as f64);
        let ground = ground_under(&at, &access);
        let deep = penetration(&at, ground).is_some_and(|deep| deep > limit) && !nearest(&at);
        let at_solid = solid(&at, ground);
        // The last clear sample, not the crossing: on the boundary, the next frame's
        // rounding would decide which side the eye is on.
        if deep || crossed(last_solid, at_solid, &at, &access) {
            return last;
        }
        (last, last_solid) = (at, at_solid);
    }
    // So an unobstructed step is exactly what the control asked for.
    *to
}

/// The azimuth goes the short way round, as the step damps it.
fn between(from: &ViewPose, to: &ViewPose, t: f64) -> ViewPose {
    let turn = wrap(
        to.azimuth - from.azimuth,
        -core::f64::consts::PI,
        core::f64::consts::PI,
    );
    ViewPose {
        pivot: std::array::from_fn(|k| from.pivot[k] + (to.pivot[k] - from.pivot[k]) * t),
        dist: from.dist + (to.dist - from.dist) * t,
        polar: from.polar + (to.polar - from.polar) * t,
        azimuth: from.azimuth + turn * t,
    }
}

fn in_solid(at: &ViewPose, access: &World) -> bool {
    solid(at, ground_under(at, access))
}

/// Whether `at`'s eye is under the top of the ground under it (`ground_under`).
fn solid(at: &ViewPose, ground: Option<f64>) -> bool {
    ground.is_some_and(|top| top > eye(at).0[2])
}

/// Whether the eye comes out of something solid at `at`, from `last`,
/// where it does not see the pivot: through a wall to its far side, which a
/// wall one or two cells thick would otherwise let it reach; `last_solid` and `at_solid` whether
/// each is in something solid (`in_solid`), as the sampling loop holds them.
fn crossed(last_solid: bool, at_solid: bool, at: &ViewPose, access: &World) -> bool {
    last_solid && !at_solid && toward_eye(at, at.dist, access).is_some()
}

/// A zoom in whose way hit something (`Step::Dive`), at the pitch asked: the
/// eye goes through what is on the line to the pivot's side, as far from the
/// pivot as the zoom asked or that leaves, and eases there through it, no
/// nearer than the orbit comes (`MIN_DIST`). None where nothing on the line
/// stands in the way.
fn dive(to: &ViewPose, climbed: &ViewPose, from: &ViewPose, access: &World) -> Option<ViewPose> {
    let roof = toward_eye(climbed, from.dist, access)?;
    // Under it, the near plane's own distance, so the near plane never cuts it.
    let dist = to.dist.min(roof.t - NEAR).max(MIN_DIST);
    Some(ViewPose { dist, ..*climbed })
}

/// What a control's step does in `climb`, one of these (`Step::of`).
enum Step {
    /// As asked: the distance granted, the drawn pitch climbing over what is
    /// in the way (`climb_polar`).
    Normal,
    /// Its way hit something, the climb's own included, on a zoom out, a tilt
    /// or a turn: as far as it got, nothing gone through.
    Stop(ViewPose),
    /// Its way hit something on a zoom in: through it, on its line (`dive`).
    Dive,
}

impl Step {
    /// The way is the straight line in control space, as the damping follows
    /// it, each sample drawn as the climb draws it and the climb's move from
    /// the sample before walked too. An eye in something solid is free, or
    /// there is no way out, but where the nearest stood it there (`nearest`),
    /// whose way out is a tilt.
    fn of(to: &ViewPose, from: &ViewPose, access: &World) -> Step {
        if nearest(to) {
            return Step::Normal;
        }
        let mut drawn = climb_pose(*from, access).0;
        if in_solid(&drawn, access) {
            return match nearest(from) {
                true => Step::Stop(*from),
                false => Step::Normal,
            };
        }
        let mut last = *from;
        for i in 1..=COLLISION_STEP_SAMPLES {
            let at = between(from, to, f64::from(i) / f64::from(COLLISION_STEP_SAMPLES));
            let next = climb_pose(at, access).0;
            if !way_clear(&drawn, &next, access) {
                return match to.dist < from.dist {
                    true => Step::Dive,
                    false => Step::Stop(last),
                };
            }
            (last, drawn) = (at, next);
        }
        Step::Normal
    }
}

/// Whether the eye gets from `from` to `to`, each as the climb draws it,
/// through nothing solid: where it lands, and on the way where the climb
/// moved its pitch.
fn way_clear(from: &ViewPose, to: &ViewPose, access: &World) -> bool {
    /// The climb's move a step's samples, for a ceiling over a low wall.
    const PITCH_SAMPLES: u32 = 4;
    let passes = if to.polar < from.polar {
        PITCH_SAMPLES
    } else {
        1
    };
    (1..=passes).all(|i| !in_solid(&between(from, to, f64::from(i) / f64::from(passes)), access))
}

/// In `climb`, how far toward `to` the orbit gets from `from` (`Step`).
fn climb_step(to: &ViewPose, from: &ViewPose, access: &World) -> ViewPose {
    match Step::of(to, from, access) {
        Step::Normal => *to,
        Step::Stop(last) => last,
        Step::Dive => dive(to, &clamp_level(*to, access), from, access).unwrap_or(*to),
    }
}

/// What `at`'s pivot meets toward its eye within `t_max`: a roof, where the
/// eye is straight over it; within the eye's own distance, what hides the
/// pivot from it.
fn toward_eye(at: &ViewPose, t_max: f64, access: &World) -> Option<Hit> {
    let ray = Ray {
        origin: at.pivot,
        dir: orbit_eye_off(1.0, at.polar, at.azimuth),
        t_max,
    };
    access.first_hit(&ray, finest()).exact().flatten()
}

/// How far toward `to` the orbit gets from `from`, as the controls ask it.
pub fn allowed_view(to: Intent, from: Intent) -> Intent {
    let (Some(from_pose), Some(to_pose)) = (pose(from), pose(to)) else {
        return to;
    };
    let reached = allowed_step(&to_pose, &from_pose);
    Intent {
        dist: reached.dist,
        polar: reached.polar,
        azimuth: reached.azimuth,
    }
}

/// The shallowest pitch that clears terrain, solved as `acos(h / dist)`.
///
/// It stands while what it is made of stands: the pose, where its pivot is
/// on the world, and the ground its walks read (`Columns`).
pub fn climb_polar(dist: f64, polar: f64, azimuth: f64) -> f64 {
    let Some(at) = pose(Intent {
        dist,
        polar,
        azimuth,
    }) else {
        return polar;
    };
    let key = MemoKey {
        at,
        pivot: world_to_canonical(&at.pivot),
    };
    let last = CLIMB_PITCH.get(|| Memo::NONE);
    last.forget_touched();
    last.of(World::now(), key, |w| climb_pose(at, &w).0.polar)
}

/// What a climb is made of, by value.
#[derive(Clone, Copy, PartialEq)]
struct MemoKey {
    at: ViewPose,
    pivot: Q3,
}

/// The last climb's pitch, kept while what it is made of stands and no
/// change touched what it read (`Memo`).
static CLIMB_PITCH: Global<Memo<MemoKey, f64>> = Global::new();

/// `at` climbed, and whether it is under a roof over the pivot: steepened as
/// the floor under the eye asks (`steepened`), the pitch kept however far
/// that got; failed only where even straight down the eye at `at`'s distance
/// is in something solid, a tunnel's roof or a cave's, and there the pitch is
/// the one asked. Ground alone never roofs the pivot. An eye in something
/// solid at the nearest stands as asked (`nearest`).
fn climb_pose(at: ViewPose, access: &World) -> (ViewPose, bool) {
    let at = clamp_level(at, access);
    if nearest(&at) && in_solid(&at, access) {
        return (at, false);
    }
    let climbed = steepened(at, access);
    match in_solid(&climbed, access) && in_solid(&ViewPose { polar: 0.0, ..at }, access) {
        true => (at, true),
        false => (climbed, false),
    }
}

/// `at` asked past the pivot's level over a floor that asks it to rise, held
/// level: the turn over the level goes on to it, and no further. Past it over
/// a floor under the pivot, as far as that floor lets it (`steepened`).
fn clamp_level(at: ViewPose, access: &World) -> ViewPose {
    let past = at.polar > core::f64::consts::FRAC_PI_2;
    let rises = |ground: f64| ground + EYE_CLEARANCE_U > at.pivot[2];
    match past && ground_under(&at, access).is_some_and(rises) {
        true => ViewPose {
            polar: core::f64::consts::FRAC_PI_2,
            ..at
        },
        false => at,
    }
}

/// `at` steepened by the rise to the floor under its eye (`climb_rise`), only
/// ever steepened, until the eye clears that floor: past level too, where
/// the floor lies under the pivot and the rise is none or below it.
fn steepened(at: ViewPose, access: &World) -> ViewPose {
    let mut climbed = at;
    for _ in 0..CLIMB_PASSES {
        let Some(rise) = climb_rise(&climbed, access) else {
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
#[path = "collision.test.rs"]
mod test;
