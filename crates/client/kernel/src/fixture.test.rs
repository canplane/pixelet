// pxl-kernel/fixture.test.rs
//
// What the tests share: an eye placed by the orbit's controls.

use pxl_spec::space::coord::v3::{add3, V3};

pub fn place(center: V3, dist: f64, polar: f64, azimuth: f64) {
    use crate::control::camera::controller::{camera, set_eye_world};
    use crate::control::camera::orbit::{orbit, orbit_eye_offset};
    use crate::control::camera::state::{camera_state, Intent};
    orbit().dist = dist;
    let cam = camera();
    (cam.polar, cam.azimuth) = (polar, azimuth);
    camera_state().intent = Intent { dist, azimuth, polar };
    let e = orbit_eye_offset(dist, polar, azimuth);
    set_eye_world(&add3(&center, &e));
}
