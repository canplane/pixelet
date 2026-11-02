// pxl-kernel/domain/control/camera/orbit/collision.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short.

use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::GROUND;
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
use pxl_spec::space::coord::zoom::units;
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

/// Rise to the floor under the eye, a cell over it (`EYE_CLEARANCE_U`), as
/// `ground_under` reads it from the eye's own cell: ground and what is
/// placed alike, at any distance. None where, `cramped`, the eye may stand
/// as it is (`embedded`), or nothing is known under it.
fn climb_rise(at: &ViewPose, access: &World, cramped: bool) -> Option<f64> {
    if cramped && embedded(&eye(at).1, access) {
        return None;
    }
    Some(ground_under(at, access)? + EYE_CLEARANCE_U - at.pivot[2])
}

/// Positive inside the terrain, off the ground under its eye
/// (`ground_under`).
fn penetration(at: &ViewPose, ground: Option<f64>) -> Option<f64> {
    let rise = ground? + EYE_CLEARANCE_U - at.pivot[2];
    Some(rise - (eye(at).0[2] - at.pivot[2]))
}

/// The eye, local and canonical.
fn eye(at: &ViewPose) -> (V3, Q3) {
    let off = orbit_eye_off(at.dist, at.polar, at.azimuth);
    let world: V3 = v3::add(&at.pivot, &off);
    (world, world_to_canonical(&world))
}

/// A step may not end deeper in the ground than it started, but for the
/// first cell of a wall or a roof (`embedded`).
fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    // Checked first so the setting skips the floor's walk.
    match settings_store().orbit_obstacle {
        OrbitObstacleMode::through => return *to,
        OrbitObstacleMode::climb => return climb_step(to, from, &World::now()),
        OrbitObstacleMode::stop => {}
    }

    let access = World::now();
    // An embedded start is where the eye may stand, not a start to climb
    // out of: from it, the ground is as deep as from the air.
    let from_ground = ground_under(from, &access);
    let limit = match embedded(&eye(from).1, &access) {
        true => 0.0,
        false => penetration(from, from_ground).map_or(0.0, |start| start.max(0.0)),
    };

    // Each sample's ground asked once, whether it is deep and in something
    // solid both read off it, and the last's kept for the next.
    let (mut last, mut last_solid) = (*from, solid(from, from_ground));
    for i in 1..=COLLISION_STEP_SAMPLES {
        let at = between(from, to, i as f64 / COLLISION_STEP_SAMPLES as f64);
        let ground = ground_under(&at, &access);
        let deep = penetration(&at, ground).is_some_and(|deep| deep > limit)
            && !embedded(&eye(&at).1, &access);
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

/// Whether the eye at `eye` stands in a solid cell with air beside it or
/// under it: the first cell of a wall, a roof or a slope's step, where it
/// may stand embedded, as the eye's rays do not meet the cell they start in
/// (pxl-engine `cast::walk`). Not a cell with air only over it, so over
/// open ground the eye keeps its height (`EYE_CLEARANCE_U`). A point is in
/// something solid as `in_solid` asks it, a cell's side away; where that is
/// not known, not embedded.
fn embedded(eye: &Q3, access: &World) -> bool {
    let side = q3::from_real(units(GROUND.cell()));
    let solid = |d: [i64; 3]| {
        let at = world::at(
            i64::from(eye[0]) + d[0] * side,
            i64::from(eye[1]) + d[1] * side,
            i64::from(eye[2]) + d[2] * side,
        );
        let z = q3::to_real(i64::from(at[2]));
        access
            .floor_under(&at, LEAF_ZOOM)
            .exact()
            .map(|top| top.is_some_and(|top| top > z))
    };
    let open = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, -1]];
    solid([0, 0, 0]) == Some(true) && open.into_iter().any(|d| solid(d) == Some(false))
}

/// Whether the eye comes out of something solid at `at`, from `last`,
/// where it does not see the pivot: through a wall it stood in the first cell
/// of (`embedded`) to the wall's far side, which a wall one or two cells
/// thick would otherwise let it reach; `last_solid` and `at_solid` whether
/// each is in something solid (`in_solid`), as the sampling loop holds them.
fn crossed(last_solid: bool, at_solid: bool, at: &ViewPose, access: &World) -> bool {
    last_solid && !at_solid && toward_eye(at, at.dist, access).is_some()
}

/// A zoom in where the climb failed (`climb_pose`), at the pitch asked, blocked
/// by what lies between the pivot and the eye (a tunnel's roof, a cave's):
/// the eye goes through it to the pivot's side on that line, as far
/// from the pivot as the zoom asked or the roof leaves, and eases there
/// through it. Where the roof is nearer the pivot than the orbit comes
/// (`MIN_DIST`), the eye stands at `MIN_DIST` in the roof's lowest cell,
/// which its rays do not meet (pxl-engine `cast::walk`): it sees the
/// pivot as from under the roof. Only a zoom in: a zoom out goes through
/// (`climb_step`). None where nothing on the line roofs the pivot, or
/// `MIN_DIST` is past the roof's lowest cell.
fn dive(to: &ViewPose, climbed: &ViewPose, from: &ViewPose, access: &World) -> Option<ViewPose> {
    if to.dist >= from.dist {
        return None;
    }
    let roof = toward_eye(climbed, from.dist, access)?;
    // Under it, the near plane's own distance, so the near plane never cuts it.
    let dist = to.dist.min(roof.t - NEAR).max(MIN_DIST);
    under(&roof, climbed, dist).then_some(ViewPose { dist, ..*climbed })
}

/// In `climb`, how far toward `to` the orbit gets from `from`: all the way,
/// the drawn pitch climbing over what is in the way (`climb_polar`), but
/// for a zoom in where the climb fails, under a roof over the pivot, which
/// goes under the roof on its line (`dive`). A zoom out is never held: under
/// a roof it goes through it, as a zoom in does.
fn climb_step(to: &ViewPose, from: &ViewPose, access: &World) -> ViewPose {
    match climb_pose(*to, access) {
        (climbed, true) => dive(to, &climbed, from, access).unwrap_or(*to),
        _ => *to,
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

/// Whether the eye `dist` from `at`'s pivot is under `roof`: short of it, or
/// in its lowest cell, which the eye's rays do not meet (pxl-engine
/// `cast::walk`), on its floor and not its ceiling as the walk's `inside` is.
fn under(roof: &Hit, at: &ViewPose, dist: f64) -> bool {
    let eye = v3::add(&at.pivot, &orbit_eye_off(dist, at.polar, at.azimuth));
    let b = roof.cube;
    dist < roof.t || (0..3).all(|k| b.min[k] <= eye[k] && eye[k] < b.max[k])
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
    let climb = Climb {
        at,
        pivot: world_to_canonical(&at.pivot),
    };
    let last = CLIMB_PITCH.get(|| Memo::NONE);
    last.forget_touched();
    last.of(World::now(), climb, |w| climb_pose(at, &w).0.polar)
}

/// What a climb is made of, by value.
#[derive(Clone, Copy, PartialEq)]
struct Climb {
    at: ViewPose,
    pivot: Q3,
}

/// The last climb's pitch, kept while what it is made of stands and no
/// change touched what it read (`Memo`).
static CLIMB_PITCH: Global<Memo<Climb, f64>> = Global::new();

/// `at` climbed, and whether it is under a roof over the pivot: steepened as
/// the floor under the eye asks (`steepened`), the pitch kept however far
/// that got; failed only where even straight down the eye at `at`'s distance
/// is in something solid it may not stand in, a tunnel's roof or a cave's,
/// and there the pitch is the one asked, for a zoom in to go under it on
/// that line (`dive`). Ground alone never roofs the pivot. The eye may stand
/// in the first cell of a wall or a roof (`embedded`) only where it is
/// `cramped`, with no room even at the orbit's nearest; with room there, at
/// any distance it climbs out of a wall rather than into it.
fn climb_pose(at: ViewPose, access: &World) -> (ViewPose, bool) {
    let cramped = cramped(&at, access);
    let stands = |p: &ViewPose| !in_solid(p, access) || (cramped && embedded(&eye(p).1, access));
    let climbed = steepened(at, access, cramped);
    match stands(&climbed) || stands(&ViewPose { polar: 0.0, ..at }) {
        true => (climbed, false),
        false => (at, true),
    }
}

/// `at` steepened by the rise to the floor under its eye (`climb_rise`), only
/// ever steepened, until the eye clears that floor: past level too, where
/// the floor lies under the pivot and the rise is none or below it.
fn steepened(at: ViewPose, access: &World, cramped: bool) -> ViewPose {
    let mut climbed = at;
    for _ in 0..CLIMB_PASSES {
        let Some(rise) = climb_rise(&climbed, access, cramped) else {
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

/// Whether the eye has no room even at the orbit's nearest: at `MIN_DIST`
/// on `at`'s line, steepened as far as it goes, still in something solid.
fn cramped(at: &ViewPose, access: &World) -> bool {
    let nearest = ViewPose {
        dist: MIN_DIST,
        ..*at
    };
    in_solid(&steepened(nearest, access, false), access)
}

#[cfg(test)]
#[path = "collision.test.rs"]
mod test;
