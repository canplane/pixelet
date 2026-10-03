// pxl-kernel/control/camera/orbit/terrain_gate.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way. The controls keep what was asked; the drawn orbit stops short.

use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::wrap;

use crate::control::camera::config::camera_cfg::view::EYE_CLEARANCE_U;
use crate::control::camera::orbit::orbit_eye_offset;
use crate::control::navigation::support::{in_cell, passage, Passage};
use crate::shell::setting::store::{settings_store, ObstacleMode};
use crate::world::access::{Access, Answer};
use crate::world::frame::coord::{local_height, local_world_to_canonical};

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
    /// The floating origin `target` is expressed against.
    pub origin: IVec,
    pub target: V3,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

/// Both ends of a step must share one origin, or the walk between them
/// crosses frames.
pub fn pose(view: ViewTriple) -> Option<ViewPose> {
    Some(ViewPose {
        origin: crate::world::frame::view::view().origin,
        target: crate::control::camera::controller::center_local()?,
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// None where no page is resident. Asked from the eye's own cell, so a cave's
/// floor counts where the eye is in the cave.
fn required_rise(at: &ViewPose) -> Option<f64> {
    let (_, eye) = eye(at);
    let ground = local_height(Access::now().floor_under(&eye).value()?, at.origin);
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// Rise to the ground `passage` reaches under the eye; none where blocked.
fn climb_rise(at: &ViewPose) -> Option<f64> {
    let (local, eye) = eye(at);
    let mut center = Pvf3::default();
    local_world_to_canonical(&mut center, &at.target, at.origin);
    let access = Access::now();
    let ground = match passage(&center, [local[0] - at.target[0], local[1] - at.target[1]], CELL.index_of(&eye)[2]) {
        Passage::Open(cz) => access.floor_under(&in_cell(&eye, cz)),
        Passage::Unknown => access.floor_under(&eye),
        Passage::Blocked => return None,
    };
    let ground = match ground {
        Answer::Exact(top) | Answer::Coarser(top, _) => top,
        Answer::Nothing(_) => return None,
    };
    Some(local_height(ground, at.origin) + EYE_CLEARANCE_U - at.target[2])
}

/// Positive inside the terrain.
fn penetration(at: &ViewPose) -> Option<f64> {
    Some(required_rise(at)? - (eye(at).0[2] - at.target[2]))
}

/// The eye, local and canonical.
fn eye(at: &ViewPose) -> (V3, Pvf3) {
    let offset = orbit_eye_offset(at.dist, at.polar, at.azimuth);
    let local: V3 = std::array::from_fn(|k| at.target[k] + offset[k]);
    let mut eye = Pvf3::default();
    local_world_to_canonical(&mut eye, &local, at.origin);
    (local, eye)
}

/// Testing only where a step lands lets the camera tunnel, since the orbit's
/// step damps through everything between.
const STEP_SAMPLES: u32 = 16;

/// A step may not end deeper in the ground than it started. A blanket
/// exemption for an already-blocked start once opened the gate for good.
pub fn allowed_step(to: &ViewPose, from: &ViewPose) -> ViewPose {
    // Checked first so the setting skips the floor's walk.
    match settings_store().obstacle {
        ObstacleMode::through => return *to,
        // A climb that would pass through something solid (a tunnel's roof to the
        // surface over it) is held as `stop` holds it.
        ObstacleMode::climb => {
            if path_clear(from, &climbed(*to)) {
                return *to;
            }
        }
        ObstacleMode::stop => {}
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
        origin: from.origin,
        target: std::array::from_fn(|k| from.target[k] + (to.target[k] - from.target[k]) * t),
        dist: from.dist + (to.dist - from.dist) * t,
        polar: from.polar + (to.polar - from.polar) * t,
        azimuth: from.azimuth + turn * t,
    }
}

fn in_solid(at: &ViewPose) -> bool {
    let (local, eye) = eye(at);
    Access::now().floor_under(&eye).value().is_some_and(|top| local_height(top, at.origin) > local[2])
}

/// What lies between the eye and the map center is not asked: that hides the
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
