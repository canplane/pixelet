// pxl-kernel/control/camera/orbit/terrain_gate.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short.

use pxl_spec::space::coord::q3::{self, Q3};
use pxl_spec::space::coord::v3::{self, V3};
use pxl_spec::space::coord::world;
use pxl_spec::space::layer::lattice::GROUND;
use xpute_guest::global::Global;
use xpute_kit::math::scalar::wrap;

use crate::control::camera::orbit::orbit_eye_offset;
use crate::param::EYE_CLEARANCE_U;
use crate::query::{over, World};
use crate::setting::store::{settings_store, OrbitObstacleMode};
use crate::view::frame::coord::world_to_canonical;
use pxl_spec::space::coord::zoom::{index_of, units};
use pxl_spec::space::layer::tile::LEAF_ZOOM;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTriple {
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

/// The target is part of the pose: the eye is `target + offset`, and a gate
/// that read only the offset could be walked through by panning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPose {
    pub target: V3,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

pub fn pose(view: ViewTriple) -> Option<ViewPose> {
    Some(ViewPose {
        target: crate::control::camera::controller::focus_world()?,
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// None where no tile is resident. Asked from the eye's own cell, so a cave's
/// floor counts where the eye is in the cave.
fn required_rise(at: &ViewPose) -> Option<f64> {
    let (_, eye) = eye(at);
    let ground = World::now().floor_under(&eye, LEAF_ZOOM).value().flatten()?;
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// Rise to the ground under the eye, reached by a step-slide at the eye's
/// height that climbs and does not fall; none where it is slid short.
fn climb_rise(at: &ViewPose) -> Option<f64> {
    let (_, eye) = eye(at);
    let center = world_to_canonical(&at.target);
    let floor = index_of(GROUND.cell(), &eye)[2] as f64 * units(GROUND.cell());
    let from = world::at(i64::from(center[0]), i64::from(center[1]), q3::from_real(over(floor)));
    let access = World::now();
    // Near, the eye's way walks the leaves; far, it is a map and goes through.
    // Where the leaf under the center, or a map's under the eye, is not held, the ground under the eye.
    let walked = access.step_slide(&from, &eye, &eye, false).unwrap_or(eye);
    if i64::from(walked[0]) != i64::from(eye[0]) || i64::from(walked[1]) != i64::from(eye[1]) {
        return None;
    }
    let ground = access.floor_under(&walked, LEAF_ZOOM).value().flatten()?;
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// Positive inside the terrain.
fn penetration(at: &ViewPose) -> Option<f64> {
    Some(required_rise(at)? - (eye(at).0[2] - at.target[2]))
}

/// The eye, local and canonical.
fn eye(at: &ViewPose) -> (V3, Q3) {
    let offset = orbit_eye_offset(at.dist, at.polar, at.azimuth);
    let world: V3 = v3::add(&at.target, &offset);
    (world, world_to_canonical(&world))
}

/// Testing only where a step lands lets the camera tunnel, since the orbit's
/// step damps through everything between.
const STEP_SAMPLES: u32 = 16;

/// A step may not end deeper in the ground than it started. A blanket
/// exemption for an already-blocked start once opened the gate for good.
pub fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    // Checked first so the setting skips the floor's walk.
    match settings_store().orbit_obstacle {
        OrbitObstacleMode::through => return *to,
        // A climb that would pass through something solid (a tunnel's roof to the
        // surface over it) is held as `stop` holds it.
        OrbitObstacleMode::climb => {
            if path_clear(from, &climbed(*to)) {
                return *to;
            }
        }
        OrbitObstacleMode::stop => {}
    }

    let limit = penetration(from).map_or(0.0, |start| start.max(0.0));

    let mut last = *from;
    for i in 1..=STEP_SAMPLES {
        let at = between(from, to, i as f64 / STEP_SAMPLES as f64);
        // The last clear sample, not the crossing: on the boundary, the next frame's
        // rounding would decide which side the eye is on.
        if penetration(&at).is_some_and(|deep| deep > limit) {
            return last;
        }
        last = at;
    }
    // So an unobstructed step is exactly what the control asked for.
    *to
}

/// The azimuth goes the short way round, as the step damps it.
fn between(from: &ViewPose, to: &ViewPose, t: f64) -> ViewPose {
    let turn = wrap(to.azimuth - from.azimuth, -core::f64::consts::PI, core::f64::consts::PI);
    ViewPose {
        target: std::array::from_fn(|k| from.target[k] + (to.target[k] - from.target[k]) * t),
        dist: from.dist + (to.dist - from.dist) * t,
        polar: from.polar + (to.polar - from.polar) * t,
        azimuth: from.azimuth + turn * t,
    }
}

fn in_solid(at: &ViewPose) -> bool {
    let (world, eye) = eye(at);
    World::now().floor_under(&eye, LEAF_ZOOM).value().flatten().is_some_and(|top| top > world[2])
}

/// What lies between the eye and the focus is not asked: that hides the
/// center but is not the eye running into anything.
fn path_clear(from: &ViewPose, to: &ViewPose) -> bool {
    (1..=STEP_SAMPLES).all(|i| !in_solid(&between(from, to, i as f64 / STEP_SAMPLES as f64)))
}

pub fn allowed_view(to: ViewTriple, from: ViewTriple) -> ViewTriple {
    let (Some(from_pose), Some(to_pose)) = (pose(from), pose(to)) else { return to };
    let reached = allowed_step(&to_pose, &from_pose);
    ViewTriple {
        dist: reached.dist,
        polar: reached.polar,
        azimuth: reached.azimuth,
    }
}

/// Steepening moves the eye and so the ground under it; two passes settle it,
/// the remainder being smaller than a cell.
const CLIMB_PASSES: u32 = 2;

/// The shallowest pitch that clears terrain, solved as `acos(h / dist)`
/// rather than scanned: a scan's fixed angles made the camera judder on flat
/// ground.
///
/// It stands while what it is made of stands: the pose, where its target is
/// on the world, and the ground its walks read (`walked`).
pub fn climb_polar(dist: f64, polar: f64, azimuth: f64) -> f64 {
    let Some(at) = pose(ViewTriple { dist, polar, azimuth }) else { return polar };
    let climb = Climb {
        at,
        center: world_to_canonical(&at.target),
    };
    let last = CLIMBED.get(|| None);
    match *last {
        Some((c, polar)) if c == climb && !crate::query::changed::within(walked(&at)) => polar,
        _ => {
            let polar = climbed(at).polar;
            *last = Some((climb, polar));
            polar
        }
    }
}

/// What a climb is made of, by value.
#[derive(Clone, Copy, PartialEq)]
struct Climb {
    at: ViewPose,
    center: Q3,
}

/// The last climb and the pitch it gave.
static CLIMBED: Global<Option<(Climb, f64)>> = Global::new();

/// The columns a climb from `at` reads, corners in units: its eyes stay on
/// the line from the target to `at`'s eye, as steepening only draws the eye
/// in, and each walk and floor reads its cells and a cell about them.
fn walked(at: &ViewPose) -> [[f64; 2]; 2] {
    let [center, eye] = [world_to_canonical(&at.target), eye(at).1].map(|q| [0, 1].map(|k| q3::to_real(i64::from(q[k]))));
    let eye = [world::nearest(eye[0], center[0]), eye[1]];
    let margin = 2.0 * units(GROUND.cell());
    [[0, 1].map(|k| center[k].min(eye[k]) - margin), [0, 1].map(|k| center[k].max(eye[k]) + margin)]
}

fn climbed(mut at: ViewPose) -> ViewPose {
    for _ in 0..CLIMB_PASSES {
        let Some(rise) = climb_rise(&at).filter(|r| *r > 0.0) else { break };
        // Only ever steepens.
        let solved = libm::acos((rise / at.dist).min(1.0));
        at.polar = at.polar.min(solved);
    }
    at
}
