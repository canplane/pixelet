// pxl-kernel/control/camera/orbit/terrain_gate.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short.

use pxl_spec::space::coord::pvf3::pvf3::Pvf3;
use pxl_spec::space::coord::v3::{add3, V3};
use pxl_spec::space::layer::lattice::GROUND;
use xpute_kit::math::scalar::wrap;

use crate::control::camera::orbit::orbit_eye_offset;
use crate::control::navigation::support::{in_cell, passage, Passage};
use crate::layer::Answer;
use crate::param::EYE_CLEARANCE_U;
use crate::query::World;
use crate::setting::store::{settings_store, OrbitObstacleMode};
use crate::view::frame::coord::world_to_canonical;
use pxl_spec::space::coord::zoom::index_of;

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
    let ground = World::now().floor_under(&eye).value().flatten()?;
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// Rise to the ground `passage` reaches under the eye; none where blocked.
fn climb_rise(at: &ViewPose) -> Option<f64> {
    let (local, eye) = eye(at);
    let center = world_to_canonical(&at.target);
    let access = World::now();
    let ground = match passage(&center, [local[0] - at.target[0], local[1] - at.target[1]], index_of(GROUND.cell(), &eye)[2]) {
        Passage::Open(cz) => access.floor_under(&in_cell(&eye, cz)),
        Passage::Unknown => access.floor_under(&eye),
        Passage::Blocked => return None,
    };
    let ground = match ground {
        Answer::Exact(Some(top)) | Answer::Coarser(Some(top), _) => top,
        Answer::Exact(None) | Answer::Coarser(None, _) | Answer::Nothing(_) => return None,
    };
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// Positive inside the terrain.
fn penetration(at: &ViewPose) -> Option<f64> {
    Some(required_rise(at)? - (eye(at).0[2] - at.target[2]))
}

/// The eye, local and canonical.
fn eye(at: &ViewPose) -> (V3, Pvf3) {
    let offset = orbit_eye_offset(at.dist, at.polar, at.azimuth);
    let world: V3 = add3(&at.target, &offset);
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
    World::now().floor_under(&eye).value().flatten().is_some_and(|top| top > world[2])
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
pub fn climb_polar(dist: f64, polar: f64, azimuth: f64) -> f64 {
    pose(ViewTriple { dist, polar, azimuth }).map_or(polar, |at| climbed(at).polar)
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
