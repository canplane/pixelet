// pxl_core/control/camera/terrain_gate.rs

//! How far a camera control gets before terrain is in the way.
//!
//! A pure function of the pose, applied to the values the controls write
//! rather than to the position they are damped into — dist is what the scale
//! bar reads, so a control that failed to take effect further down would move
//! the bar while the view stood still.

use pxl_shared::spec::space::indexer::L3_IDX;
use pxl_shared::spec::space::pvf3::ivec::{self, IVec};
use pxl_shared::spec::space::pvf3::pvf3::{self, PVF3};
use pxl_shared::spec::space::v3::V3;
use pxl_shared::spec::space::xy::TO_XY;
use xpute_core::kit::math::wrap;

use crate::control::camera::config::camera_cfg;
use crate::control::camera::orbit::{orbit_local_offset, rotate2};
use crate::layer::terrain::elevation::get_ele_at_local;
use crate::shell::map_store::map_store;
use crate::shell::setting::store::{settings_store, CameraTerrain};
use crate::world::frame::coord::{get_local_world_origin, local_world_to_canonical};

/// One cell, so a bicubic elevation sample disagreeing slightly with the
/// voxel it produced cannot put the eye inside a block.
const EYE_CLEARANCE_U: f64 = 1.0;

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
/// (real u against `origin_page`), the frame the orbit offset and the
/// heightfield lookup already share, and the one a step can be interpolated in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewPose {
    /// `TO_XY(L3_IDX(map_origin))` for the frame `target` is expressed in.
    pub origin_page: IVec,
    pub target: V3,
    pub dist: f64,
    pub polar: f64,
    pub azimuth: f64,
}

/// The pose these controls describe while looking at `center`, or None before
/// the first fix, when there is no frame to express it in.
///
/// Both ends of a step have to be built against one origin or the walk
/// between them crosses frames; the store's current centre supplies it for
/// every caller, and a candidate target is expressed against that same one.
pub fn pose(view: ViewTriple, center: Option<&PVF3>) -> Option<ViewPose> {
    let map_center = map_store().map_center?;
    let at = center.copied().unwrap_or(map_center);
    let origin = get_local_world_origin(Some(&map_center));
    let local = PVF3 { i: ivec::sub(at.i, origin), f: at.f };
    Some(ViewPose {
        origin_page: TO_XY(L3_IDX(origin)),
        target: [pvf3::to_real(pvf3::X(&local)), pvf3::to_real(pvf3::Y(&local)), pvf3::to_real(pvf3::Z(&local))],
        dist: view.dist,
        polar: view.polar,
        azimuth: view.azimuth,
    })
}

/// How far above the target the eye must sit to clear the ground beneath it,
/// or None when the tile under it has not loaded — an unarrived sample says
/// nothing.
fn required_rise(at: &ViewPose) -> Option<f64> {
    let o = orbit_local_offset(at.dist, at.polar);
    // The camera never rotates; azimuth spins the world beneath it, so
    // un-rotating the camera-local offset puts it in the ground's frame.
    let offset = rotate2(0.0, -o.horiz_dist, -at.azimuth);
    let ground = get_ele_at_local(at.origin_page, at.target[0] + offset.x, at.target[1] + offset.y)?;
    Some(ground + EYE_CLEARANCE_U - at.target[2])
}

/// How far below its clearance the eye sits: positive inside the terrain,
/// None where the ground is not known yet.
fn penetration(at: &ViewPose) -> Option<f64> {
    Some(required_rise(at)? - orbit_local_offset(at.dist, at.polar).vert_dist)
}

/// Testing only where a step lands lets the camera tunnel: damping.rs
/// damps toward the new state and visits everything between. The straight
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
    // Checked first so the setting skips the heightfield walk, not its result.
    // `climb` grants every control too — it answers terrain by pitching the
    // view, not by holding a control back.
    if settings_store().camera_terrain != CameraTerrain::Stop {
        return *to;
    }

    let limit = penetration(from).map_or(0.0, |start| start.max(0.0));

    // The short way round, matching how view_frame.rs damps azimuth.
    let turn = wrap(to.azimuth - from.azimuth, -core::f64::consts::PI, core::f64::consts::PI);
    let mut last = *from;
    for i in 1..=STEP_SAMPLES {
        let t = i as f64 / STEP_SAMPLES as f64;
        let at = ViewPose {
            origin_page: from.origin_page,
            target: [
                from.target[0] + (to.target[0] - from.target[0]) * t,
                from.target[1] + (to.target[1] - from.target[1]) * t,
                from.target[2] + (to.target[2] - from.target[2]) * t,
            ],
            dist: from.dist + (to.dist - from.dist) * t,
            polar: from.polar + (to.polar - from.polar) * t,
            azimuth: from.azimuth + turn * t,
        };
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

/// The orbit half, where the target holds still.
pub fn allowed_view(to: ViewTriple, from: ViewTriple) -> ViewTriple {
    let (Some(from_pose), Some(to_pose)) = (pose(from, None), pose(to, None)) else { return to };
    let reached = allowed_step(&to_pose, &from_pose);
    ViewTriple {
        dist: reached.dist,
        polar: reached.polar,
        azimuth: reached.azimuth,
    }
}

/// The pan half: the target moves and takes the eye with it.
///
/// Panning is the one continuous control that is not in the view triple, so
/// it went through no gate at all and walking into a hill sideways simply
/// worked. Jumps do not come through here — a search result or a GPS recentre
/// is a destination the user named, not a path they dragged.
pub fn allowed_pan(view: ViewTriple, to_center: &PVF3) -> PVF3 {
    let (Some(from_pose), Some(to_pose)) = (pose(view, None), pose(view, Some(to_center))) else {
        return *to_center;
    };
    let reached = allowed_step(&to_pose, &from_pose);
    if reached == to_pose {
        return *to_center;
    }
    let map_origin = get_local_world_origin(map_store().map_center.as_ref());
    let mut out = PVF3::default();
    local_world_to_canonical(&mut out, &reached.target, map_origin);
    out
}

/// Steepening the pitch moves the eye, which changes the ground under it, so
/// the answer is implicit. Two passes settle it: the eye travels less and less
/// as the pitch approaches vertical, and the remainder is smaller than the cell
/// the height was read from.
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
/// There is nearly always an answer. At MIN_POLAR the eye is directly above
/// the target and only terrain higher than `target + dist` could block it.
pub fn climb_polar(dist: f64, polar: f64, azimuth: f64) -> f64 {
    let Some(mut at) = pose(ViewTriple { dist, polar, azimuth }, None) else { return polar };
    for _ in 0..CLIMB_PASSES {
        let Some(rise) = required_rise(&at).filter(|r| *r > 0.0) else { break };
        // Only ever steepens: an eye already high enough is left where the
        // controls put it.
        let solved = (rise / dist).min(1.0).acos();
        at.polar = camera_cfg::view::MIN_POLAR.max(at.polar.min(solved));
    }
    at.polar
}
