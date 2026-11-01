// pxl-kernel/domain/control/camera/orbit/collision.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short
//! (`stop`), or in `climb` is drawn from what was asked and the world alone
//! (`climbed`).

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
/// first cell of a wall or a roof (`embedded`). A blanket exemption for an
/// already-blocked start once opened the gate for good.
fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    // Checked first so the setting skips the floor's walk.
    match settings_store().orbit_obstacle {
        OrbitObstacleMode::through => return *to,
        // What is drawn is the asked pose climbed (`climbed`), so nothing asked
        // is held back.
        OrbitObstacleMode::climb => return *to,
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

/// Whether `at`'s eye is under the top of the ground under it (`ground_under`).
fn solid(at: &ViewPose, ground: Option<f64>) -> bool {
    ground.is_some_and(|top| top > eye(at).0[2])
}

/// Whether the eye at `eye` stands in a solid cell with air beside it or
/// under it: the first cell of a wall, a roof or a slope's step, where it
/// may stand embedded, as the eye's rays do not meet the cell they start in
/// (pxl-spec-core `cast::walk`). Not a cell with air only over it, so over
/// open ground the eye keeps its height (`EYE_CLEARANCE_U`). A point is in
/// something solid as `solid` asks it, a cell's side away; where that is
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
/// each is in something solid (`solid`), as the sampling loop holds them.
fn crossed(last_solid: bool, at_solid: bool, at: &ViewPose, access: &World) -> bool {
    last_solid && !at_solid && toward_eye(at, at.dist, access).is_some()
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

/// The pose the orbit draws for `view`, asked, in `climb` (`climbed`), kept
/// while what it is made of stands: the pose, where its pivot is on the
/// world, and what its rays and floors read (`Memo`). `view` where the
/// camera stands nowhere.
pub fn climbed_view(view: Intent) -> Intent {
    let Some(at) = pose(view) else {
        return view;
    };
    let climb = Climb {
        at,
        pivot: world_to_canonical(&at.pivot),
    };
    let last = CLIMBED.get(|| Memo::NONE);
    last.forget_touched();
    let (dist, polar) = last.of(World::now(), climb, |w| {
        let c = climbed(at, &w);
        (c.dist, c.polar)
    });
    Intent {
        dist,
        polar,
        ..view
    }
}

/// What a climb is made of, by value.
#[derive(Clone, Copy, PartialEq)]
struct Climb {
    at: ViewPose,
    pivot: Q3,
}

/// The last climb's distance and pitch, kept while what it is made of
/// stands and no change touched what it read (`Memo`).
static CLIMBED: Global<Memo<Climb, (f64, f64)>> = Global::new();

/// `at` as the orbit draws it: at the distance asked, its pitch raised,
/// never lowered, until the line from the pivot to the eye is open and the
/// eye is a cell over the floor under it (`EYE_CLEARANCE_U`), whatever
/// stands in the way, ground or what is placed; each pass's pitch solved
/// from what it met (`pitch_over`, `acos` of the floor's rise), not scanned,
/// so the eye does not judder. Where even straight down does not open the
/// line, a roof is over the pivot, a tunnel's, a cave's or a cube's: the eye
/// stays on the line asked, `NEAR` short of the roof and no nearer than
/// `MIN_DIST`. Of the pose asked and the world alone, so no pose drawn
/// before holds the eye where it was.
fn climbed(at: ViewPose, access: &World) -> ViewPose {
    let mut climbed = at;
    for _ in 0..CLIMB_PASSES {
        let mut polar = climbed.polar;
        if let Some(met) = toward_eye(&climbed, climbed.dist, access) {
            match pitch_over(&met, &climbed) {
                Some(over) => polar = polar.min(over),
                None => break,
            }
        }
        let height = climbed.dist * libm::cos(climbed.polar);
        if let Some(rise) = ground_under(&climbed, access)
            .map(|g| g + EYE_CLEARANCE_U - climbed.pivot[2])
            .filter(|&rise| rise > height)
        {
            polar = polar.min(libm::acos((rise / climbed.dist).min(1.0)));
        }
        if polar == climbed.polar {
            break;
        }
        climbed.polar = polar;
    }
    if toward_eye(&climbed, climbed.dist, access).is_none() {
        return climbed;
    }
    match toward_eye(&at, at.dist, access) {
        Some(roof) => ViewPose {
            dist: (roof.t - NEAR).max(MIN_DIST).min(at.dist),
            ..at
        },
        None => at,
    }
}

/// The pitch whose line from `at`'s pivot, on its heading, passes a cell
/// over the top of what it `met` (`EYE_CLEARANCE_U`): the line rises from
/// the pivot, so it is lowest at the nearest of `met`'s box along the
/// heading. None where the line is straight down already, or `met` is not
/// over the pivot's height.
fn pitch_over(met: &Hit, at: &ViewPose) -> Option<f64> {
    let off = orbit_eye_off(1.0, at.polar, at.azimuth);
    let len = libm::hypot(off[0], off[1]);
    if len <= f64::EPSILON {
        return None;
    }
    let heading = [off[0] / len, off[1] / len];
    let b = met.cube;
    let nearest = [b.min[0], b.max[0]]
        .into_iter()
        .flat_map(|x| [b.min[1], b.max[1]].map(|y| [x, y]))
        .map(|c| (c[0] - at.pivot[0]) * heading[0] + (c[1] - at.pivot[1]) * heading[1])
        .fold(f64::INFINITY, f64::min)
        .max(0.0);
    let rise = b.max[2] + EYE_CLEARANCE_U - at.pivot[2];
    (rise > 0.0).then(|| libm::atan2(nearest, rise))
}

#[cfg(test)]
#[path = "collision.test.rs"]
mod test;
