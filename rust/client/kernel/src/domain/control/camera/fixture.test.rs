// pxl-kernel/domain/control/camera/fixture.test.rs
//
// What the camera's tests share: an eye placed by the orbit's controls.

use pxl_spec::space::coord::v3::{self, V3};

pub fn place(center: V3, dist: f64, polar: f64, azimuth: f64) {
    use crate::domain::control::camera::controller::{camera, set_eye_world};
    use crate::domain::control::camera::orbit::{orbit, orbit_eye_offset};
    use crate::domain::control::camera::state::{camera_state, Intent};
    orbit().dist = dist;
    let cam = camera();
    (cam.polar, cam.azimuth) = (polar, azimuth);
    camera_state().intent = Intent { dist, azimuth, polar };
    let e = orbit_eye_offset(dist, polar, azimuth);
    set_eye_world(&v3::add(&center, &e));
}
