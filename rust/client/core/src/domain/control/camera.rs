// pxl-core/domain/control/camera.rs

//! The camera, which is a pivot, a distance and a facing, and the controllers that move
//! it. The folder is layered, and a part names only the parts below it
//! (pxl-tool rule.rs `CAMERA_ORDER`). The camera itself, here, is at
//! the bottom. Above it are the move it is on (`motion`) and where the
//! pivot stands on the ground (`pivot`). Above that are the orbit and the
//! first-person camera, each drawing the eye about the one pivot. At the top, `controller`
//! decides which of the two moves the camera and where its pivot is. What
//! asks the camera to move lives outside the folder (domain/control/input.rs,
//! navigation) and reaches it through `controller` alone.

pub(crate) mod controller;
mod first_person;
mod motion;
mod orbit;
mod pivot;

use pxl_global::Global;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::wrap;

use crate::domain::control::param::FACING_DAMP;
use crate::domain::eye::EyePose;
use crate::domain::smooth::{smooth_angle, smooth_vec};
use crate::domain::view::frame::pose;

// A facing is a direction in spherical coordinates about the world's Z
// axis. The polar angle θ is measured from straight down and is clamped to
// [0, π]. The azimuth φ is measured from north toward east and wraps. At
// either pole φ still matters: it names the direction the facing leaves the
// pole along. The orbit adds the distance r. The camera has no roll.
fn facing_domain(polar: f64, azimuth: f64) -> (f64, f64) {
    (
        polar.clamp(0.0, core::f64::consts::PI),
        wrap(azimuth, -core::f64::consts::PI, core::f64::consts::PI),
    )
}

fn facing_dir(polar: f64, azimuth: f64) -> V3 {
    let (st, ct) = (libm::sin(polar), libm::cos(polar));
    let (sp, cp) = (libm::sin(azimuth), libm::cos(azimuth));
    [st * sp, st * cp, -ct]
}

/// At polar 0 the facing looks straight down and the azimuth points up the
/// screen. At either pole the azimuth only turns the screen about the facing.
fn facing_rot(polar: f64, azimuth: f64) -> Mat4 {
    let mut about_z = mat4::new();
    mat4::rot_z(&mut about_z, azimuth);
    let (s, c) = (libm::sin(polar), libm::cos(polar));
    let mut about_x = mat4::new();
    (about_x[5], about_x[6], about_x[9], about_x[10]) = (c, -s, s, c);
    let mut rot: Mat4 = mat4::new();
    mat4::mul(&mut rot, &about_x, &about_z);
    rot
}

/// The camera, the one there is: the pivot it turns about, how far its eye
/// stands from it where the controller has a distance, and its facing. A
/// pan writes its pivot at once, and a move takes it to its end (motion.rs
/// `Move`).
/// The eye is derived from it by whichever controller moves the camera,
/// where a flight carries it and as the ground lets it be drawn
/// (controller.rs `pose`), so a change of controller changes nothing here.
/// The pivot is kept in world doubles, not as a canonical point, which
/// rounds to 1/256: eased a frame at a time, that rounding would make it
/// drift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Camera {
    pub pivot: V3,
    pub dist: f64,
    /// Radians from straight down.
    pub polar: f64,
    /// Radians about Z from north toward east.
    pub azimuth: f64,
}

static CAMERA: Global<Option<Camera>> = Global::new();

/// The camera, none until it is first put somewhere.
fn camera() -> &'static mut Option<Camera> {
    CAMERA.get(|| None)
}

/// Eases the facing of `cam` toward `polar` and `azimuth` over `delta`
/// seconds, at one rate (`FACING_DAMP`), its azimuth the short way round.
fn ease_facing(cam: &mut Camera, polar: f64, azimuth: f64, delta: f64) {
    let [eased_polar] = smooth_vec([cam.polar], [polar], FACING_DAMP, delta);
    let eased_azimuth = smooth_angle(cam.azimuth, azimuth, FACING_DAMP, delta);
    (cam.polar, cam.azimuth) = facing_domain(eased_polar, eased_azimuth);
}

/// Whether a camera with its eye at `eye`, facing as `cam`, is still to ease
/// toward the eye `eye_to` facing `polar` and `azimuth`: whether its pose and
/// that one are distinct in the view. Where they are not, what the ease has
/// left is under what is seen, and the camera takes the pose whole.
fn eases_toward(cam: &Camera, eye: V3, eye_to: V3, polar: f64, azimuth: f64) -> bool {
    pose::distinct(
        &EyePose {
            eye,
            rot: facing_rot(cam.polar, cam.azimuth),
        },
        &EyePose {
            eye: eye_to,
            rot: facing_rot(polar, azimuth),
        },
    )
}

#[cfg(test)]
#[path = "camera/fixture.test.rs"]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "camera.test.rs"]
mod test;
