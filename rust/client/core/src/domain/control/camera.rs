// pxl-core/domain/control/camera.rs

//! The camera, which is an eye and a facing, and the controllers that move
//! it. The folder is layered, and a part names only the parts below it
//! (pxl-tool rule.rs `CAMERA_ORDER`). The camera itself, here, is at
//! the bottom, beside a gesture's anchor (`gesture`). Above them is what the
//! controls ask for (`state`). Above that are the orbit and the first-person
//! camera, and each turns about its own pivot. At the top, `controller`
//! decides which of the two moves the camera and where its pivot is. What
//! asks the camera to move lives outside the folder (domain/control/input.rs,
//! navigation).

pub mod controller;
pub mod first_person;
pub mod gesture;
pub mod orbit;
pub mod state;

use pxl_global::Global;
use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::V3;
use xpute_kit::math::scalar::wrap;

use crate::domain::control::param::{DEFAULT_AZIMUTH, DEFAULT_POLAR, EASE_EPS_RAD, ROT_DAMP};
use crate::domain::smooth::{smooth_angle, smooth_vec};

// A facing is a direction in spherical coordinates about the world's Z
// axis. The polar angle θ is measured from straight down and is clamped to
// [0, π]. The azimuth φ is measured from north toward east and wraps. At
// either pole φ still matters: it names the direction the facing leaves the
// pole along. The orbit adds the distance r. The camera has no roll.
pub fn facing_domain(polar: f64, azimuth: f64) -> (f64, f64) {
    (
        polar.clamp(0.0, core::f64::consts::PI),
        wrap(azimuth, -core::f64::consts::PI, core::f64::consts::PI),
    )
}

pub fn facing_dir(polar: f64, azimuth: f64) -> V3 {
    let (st, ct) = (libm::sin(polar), libm::cos(polar));
    let (sp, cp) = (libm::sin(azimuth), libm::cos(azimuth));
    [st * sp, st * cp, -ct]
}

/// At polar 0 the facing looks straight down and the azimuth points up the
/// screen. At either pole the azimuth only turns the screen about the facing.
pub fn facing_rot(polar: f64, azimuth: f64) -> Mat4 {
    let mut about_z = mat4::new();
    mat4::rot_z(&mut about_z, azimuth);
    let (s, c) = (libm::sin(polar), libm::cos(polar));
    let mut about_x = mat4::new();
    (about_x[5], about_x[6], about_x[9], about_x[10]) = (c, -s, s, c);
    let mut rot: Mat4 = mat4::new();
    mat4::mul(&mut rot, &about_x, &about_z);
    rot
}

/// The camera as it is drawn. The eye is kept in world doubles, not as a
/// canonical point. A canonical point rounds to 1/256, and the orbit derives
/// the eye again each frame, so that rounding would make the pivot drift.
pub struct Camera {
    pub eye: Option<V3>,
    /// Radians about Z from north toward east.
    pub azimuth: f64,
    /// Radians from straight down.
    pub polar: f64,
}

static CAMERA: Global<Camera> = Global::new();

pub fn camera() -> &'static mut Camera {
    // The camera starts where the controls' intent starts (state.rs
    // `CameraState::new`), so nothing eases on the first frame.
    CAMERA.get(|| Camera {
        eye: None,
        azimuth: DEFAULT_AZIMUTH,
        polar: DEFAULT_POLAR,
    })
}

pub fn eye_world() -> Option<V3> {
    camera().eye
}

pub fn set_eye_world(at: &V3) {
    camera().eye = Some(*at);
}

/// The azimuth the camera is drawn at, in radians from north toward east.
pub fn azimuth() -> f64 {
    camera().azimuth
}

/// The polar angle the camera is drawn at, in radians from straight down.
pub fn polar() -> f64 {
    camera().polar
}

/// Eases the facing of `cam` over `delta` seconds, its polar angle toward
/// `polar` at `polar_rate` and its azimuth toward `azimuth` the short way
/// round, and returns whether it is still moving.
pub fn ease_facing(
    cam: &mut Camera,
    polar: f64,
    polar_rate: f64,
    azimuth: f64,
    delta: f64,
) -> bool {
    let ([eased_polar], polar_moving) =
        smooth_vec([cam.polar], [polar], polar_rate, delta, EASE_EPS_RAD);
    let (eased_azimuth, azimuth_moving) =
        smooth_angle(cam.azimuth, azimuth, ROT_DAMP, delta, EASE_EPS_RAD);
    (cam.polar, cam.azimuth) = facing_domain(eased_polar, eased_azimuth);
    polar_moving || azimuth_moving
}

#[cfg(test)]
#[path = "camera/fixture.test.rs"]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "camera.test.rs"]
mod test;
