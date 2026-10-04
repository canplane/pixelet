// pxl-kernel/control/camera/orbit/terrain_gate.rs

//! How far the orbit gets toward what the controls ask before terrain is in
//! the way: a pure function of the pose, from where the orbit is drawn to
//! where the controls would have it (orbit.rs `ask`). The controls keep
//! what was asked; the drawn orbit stops short, and what reads the zoom reads
//! the drawn radius, which stands still with the view.

use pxl_spec::space::pvf3::ivec::IVec;
use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pvf3::pvf3::Pvf3;
use pxl_spec::space::v3::V3;
use xpute_core::math::scalar::wrap;

use crate::control::camera::config::camera_cfg::view::EYE_CLEARANCE_U;
use crate::control::camera::orbit::orbit_eye_offset;
use crate::control::navigation::support::{in_cell, passage, Passage};
use crate::shell::setting::store::{settings_store, OrbitObstacleMode};
use crate::world::access::{Access, Answer};
use crate::world::frame::coord::{local_height, local_world_origin, local_world_to_canonical};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTriple {
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

/// Everything that decides where the eye is: the point it looks at, and the
/// orbit around it.
///
/// The target belongs here because the eye is `target + orbit offset` and a
/// gate that reads only the second term can be walked through by moving the
/// first — which is what panning does. It is carried in the local render frame
/// (real u against `origin`), the frame the orbit offset and the floor
/// under the eye are read in, and the one a step can be interpolated in.
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
        origin: local_world_origin(),
        target: crate::control::camera::controller::center_local()?,
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// How far above the target the eye must sit to clear what is under it,
/// or None where no page over it is resident — an unarrived sample says
/// nothing. What is under it is asked from the eye's own cell, as a walker's
/// footing is (world/access.rs `floor_under`): a cave's floor where the eye
/// is in the cave, the top of what it is inside where it is in the ground —
/// never the surface's height alone, which a cave is under.
fn required_rise(at: &ViewPose) -> Option<f64> {
    let (_, eye) = eye(at);
    let ground = local_height(Access::now().floor_under(&eye).value()?, at.origin);
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// The rise a climb needs: the ground under the eye on the layer the way
/// from the map center reaches it by (`passage`), not the top of whatever
/// fills its column. None where the way is blocked, so the climb holds.
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

/// Testing only where a step lands lets the camera tunnel: the orbit's step
/// (orbit.rs `turn`) damps toward the new state and visits everything
/// between. The straight
/// line in pose space is the path it takes.
const STEP_SAMPLES: u32 = 16;

/// As far along `from` -> `to` as terrain allows, in `stop` mode.
///
/// The rule is that a step may not end up deeper in the ground than it
/// started. Outside terrain that is the plain "refuse to enter"; inside it,
/// every escape is still granted and the bar tightens on its own as one
/// succeeds. A blanket exemption for an already-blocked start opened the gate
/// completely and left it open, since nothing that followed could ever close
/// it again.
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
        // The last clear sample, not the crossing point: landing on the boundary
        // leaves the next frame's rounding to decide which side the eye is on.
        if penetration(&at).is_some_and(|deep| deep > limit) {
            return last;
        }
        last = at;
    }
    // `to` itself rather than the t=1 sample, so an unobstructed step is the
    // exact values the control asked for.
    *to
}

/// The pose `t` of the way from `from` to `to`: the straight line in pose
/// space the orbit's step takes, the azimuth the short way round as the step
/// damps it.
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

/// Whether the eye of `at` is inside what fills the world: the floor under
/// it, asked from its own cell (`required_rise`), standing over it. Where
/// the ground is not known yet nothing says it is.
fn in_solid(at: &ViewPose) -> bool {
    let (local, eye) = eye(at);
    Access::now().floor_under(&eye).value().is_some_and(|top| local_height(top, at.origin) > local[2])
}

/// Whether the eye goes from `from`'s to `to`'s through nothing solid, along
/// the path the orbit's step takes. What lies between the eye and the map
/// center is not asked: that hides the map center, and is not the eye
/// running into anything.
fn path_clear(from: &ViewPose, to: &ViewPose) -> bool {
    (1..=STEP_SAMPLES).all(|i| !in_solid(&between(from, to, i as f64 / STEP_SAMPLES as f64)))
}

/// The orbit half, where the target holds still.
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

/// The shallowest pitch that clears terrain, between `polar` and straight
/// down, or straight down if nothing does.
///
/// Solved rather than searched. The eye sits `dist * cos(polar)` above the
/// target, so clearing a required rise `h` needs `acos(h / dist)` — smooth in
/// `h`, which matters more than it sounds: a scan returns one of a fixed set of
/// angles, so its output steps even where the terrain does not, and the camera
/// judders on flat ground it is merely passing over.
///
/// There is nearly always an answer. Straight down the eye is directly above
/// the target and only terrain higher than `target + dist` could block it.
pub fn climb_polar(dist: f64, polar: f64, azimuth: f64) -> f64 {
    pose(ViewTriple { dist, polar, azimuth }).map_or(polar, |at| climbed(at).polar)
}

/// `at` with its pitch steepened as `climb_polar` solves it.
fn climbed(mut at: ViewPose) -> ViewPose {
    for _ in 0..CLIMB_PASSES {
        let Some(rise) = climb_rise(&at).filter(|r| *r > 0.0) else { break };
        // Only ever steepens.
        let solved = libm::acos((rise / at.dist).min(1.0));
        at.polar = at.polar.min(solved);
    }
    at
}
