// pxl-core/domain/control/camera/fixture.test.rs
//
// What the camera's tests share, and how a test outside the folder places
// and reads the camera past its controller. `place` stands the camera at a
// pivot on no move.

use pxl_spec::space::coord::v3::V3;

use crate::domain::control::camera::motion::{end, motion};
use crate::domain::control::camera::{Camera, camera, orbit};

pub(crate) fn place(at: V3, dist: f64, polar: f64, azimuth: f64) {
    *motion() = None;
    orbit::level();
    *camera() = Some(Camera {
        pivot: at,
        dist,
        polar,
        azimuth,
    });
}

/// The camera as it is now, before a flight carries it and the ground bounds
/// what is drawn of it.
pub(crate) fn camera_now() -> Camera {
    (*camera()).expect("a camera")
}

/// Where the camera ends: its distance, azimuth and polar angle.
pub(crate) fn end_now() -> (f64, f64, f64) {
    let m = end(&camera_now());
    (m.dist, m.azimuth, m.polar)
}

/// Leaves the camera never put, as at the app's start.
pub(crate) fn clear_camera() {
    *camera() = None;
    *motion() = None;
    orbit::level();
}
