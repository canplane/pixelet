// pxl-core/domain/control/camera/fixture.test.rs
//
// What the camera's tests share. `place` stands the orbit at a pivot with
// its intent and its drawn camera already equal, so nothing eases.

use pxl_spec::space::coord::v3::{self, V3};

pub(crate) fn place(at: V3, dist: f64, polar: f64, azimuth: f64) {
    use crate::domain::control::camera::orbit::{orbit, orbit_eye_off};
    use crate::domain::control::camera::state::{Intent, camera_state_mut};
    use crate::domain::control::camera::{camera, set_eye_world};
    use crate::domain::view::frame::coord::world_to_canonical;
    camera_state_mut().to = Some(world_to_canonical(&at));
    orbit().dist = dist;
    let cam = camera();
    (cam.polar, cam.azimuth) = (polar, azimuth);
    camera_state_mut().intent = Intent {
        dist,
        azimuth,
        polar,
    };
    let e = orbit_eye_off(dist, polar, azimuth);
    set_eye_world(&v3::add(&at, &e));
}
