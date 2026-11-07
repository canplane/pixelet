// pxl-kernel/domain/control/camera.rs

//! The camera, an eye and its facing, and the controllers that move it, in
//! one order: the camera itself here, under everything in the folder; what
//! the controls ask (`state`) and a gesture's anchor (`gesture`); the orbit
//! and the first-person camera, each turning about its own pivot; and over
//! them `controller`, which one moves it and where its pivot is. What asks
//! them to is above it (domain/control/input.rs, navigation).

pub mod controller;
pub mod first_person;
pub mod gesture;
pub mod orbit;
pub mod state;

use pxl_spec::space::coord::mat4::{self, Mat4};
use pxl_spec::space::coord::v3::V3;
use xpute_core::global::Global;
use xpute_kit::math::scalar::wrap;

use crate::domain::control::param::{DEFAULT_AZIMUTH, DEFAULT_POLAR};

// A facing in spherical coordinates about the world's Z, θ = polar and
// φ = azimuth: θ from straight down, clamped to [0, π]; φ from north toward
// east, wrapped. At either pole φ still names the direction the facing returns
// along. The orbit adds r = dist. There is no roll control.
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

/// Straight down at polar 0 with the azimuth up the screen; at the poles the
/// azimuth only turns the screen about the facing.
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

/// The eye is kept in the world's doubles: a canonical point's 1/256 rounding
/// held in it would walk the pivot as the orbit re-derives the eye each
/// frame.
pub struct Camera {
    pub eye: Option<V3>,
    /// Heading about Z, and the angle down from vertical.
    pub azimuth: f64,
    pub polar: f64,
}

static CAMERA: Global<Camera> = Global::new();

pub fn camera() -> &'static mut Camera {
    // Where the controls start, so nothing eases at the first frame.
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

/// The drawn azimuth, radians from north toward east.
pub fn azimuth() -> f64 {
    camera().azimuth
}

/// Radians from straight down.
pub fn polar() -> f64 {
    camera().polar
}

#[cfg(test)]
#[path = "camera/fixture.test.rs"]
pub(crate) mod fixture;

#[cfg(test)]
#[path = "camera.test.rs"]
mod test;
