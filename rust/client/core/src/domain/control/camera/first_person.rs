// pxl-core/domain/control/camera/first_person.rs

//! The first-person camera turns about its eye, which stands over its pivot
//! (`eye_off`): the pivot is where a body stands over its floor, and the eye
//! half a cell under the top of the body's box (query/support.rs `over`, `eye_over`). No
//! control sets the height. A pan moves the pivot at once, and its height
//! follows on a move (motion.rs `Move`), so a step up or down is eased. The
//! eye never leaves the body: a place past where the eye draws the ground as
//! it is is not flown to. The camera warps there (controller.rs `go`).

use pxl_spec::space::coord::v3::{self, V3};

use crate::domain::control::camera::motion::Move;
use crate::domain::control::camera::{Camera, ease_facing, eases_toward};
use crate::domain::param::POS_DAMP;
use crate::domain::query::{eye_over, over};
use crate::domain::smooth::smooth_vec;

/// The eye's place over the pivot, from where a body stands to where its eye
/// does.
fn eye_off() -> V3 {
    [0.0, 0.0, eye_over(0.0) - over(0.0)]
}

/// Where the eye of `cam` stands: over its pivot.
pub(crate) fn eye(cam: &Camera) -> V3 {
    v3::add(&cam.pivot, &eye_off())
}

/// Moves `cam` along the move `mv` over `delta` seconds, its pivot and its
/// facing eased, and returns whether it is still moving.
pub(crate) fn step(cam: &mut Camera, mv: &Move, delta: f64) -> bool {
    let to = mv.to;
    // First person draws no distance, so the one a move ends at is taken at
    // once, for the orbit to find when it draws the camera again.
    cam.dist = mv.dist;
    let moving = eases_toward(
        cam,
        eye(cam),
        v3::add(&to, &eye_off()),
        mv.polar,
        mv.azimuth,
    );
    if moving {
        ease_facing(cam, mv.polar, mv.azimuth, delta);
        cam.pivot = smooth_vec(cam.pivot, to, POS_DAMP, delta);
    } else {
        (cam.pivot, cam.polar, cam.azimuth) = (to, mv.polar, mv.azimuth);
    }
    moving
}
