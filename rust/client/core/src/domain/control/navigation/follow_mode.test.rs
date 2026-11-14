// pxl-core/domain/control/navigation/follow_mode.test.rs

use core::f64::consts::PI;

use xpute_kit::math::scalar::wrap;

use super::*;
use crate::domain::control::camera::camera;
use crate::domain::control::camera::fixture::place;
use crate::domain::control::camera::orbit;
use crate::domain::control::sensor::reading::{
    device_location_store_mut, device_orient_store_mut, SensorState,
};

/// Following turns the view all the way to each heading the store keeps,
/// through the camera's own easing, however little past the store's dead
/// zone each heading moves.
#[test]
fn following_turns_all_the_way_to_the_heading() {
    place([0.0, 0.0, 10.0], 400.0, 1.0, 0.0);
    device_location_store_mut().state = SensorState::of(SensorStatus::Ready);
    set_following(true);
    for k in 1..=10 {
        let heading = 1.6 * f64::from(k);
        assert!(device_orient_store_mut().set_orient(Some(heading)));
        follow_mode_step();
        let eased = (0..10_000).find(|_| !orbit::step(1.0 / 60.0));
        assert!(eased.is_some(), "the camera rests");
        let off = wrap(camera().azimuth - heading.to_radians(), -PI, PI);
        assert!(
            off.abs() < 1e-9,
            "{heading}° rests {}° short",
            off.to_degrees()
        );
    }
}
