// pxl-core/domain/control/navigation/follow_mode.test.rs

use core::f64::consts::PI;

use xpute_kit::math::scalar::wrap;

use super::*;
use crate::domain::control::camera::camera;
use crate::domain::control::camera::fixture::place;
use crate::domain::control::camera::orbit;

/// Following turns the view all the way to each heading the UI tells,
/// through the camera's own easing, however little each heading moves.
#[test]
fn following_turns_all_the_way_to_the_heading() {
    place([0.0, 0.0, 10.0], 400.0, 1.0, 0.0);
    sensor::set_location_status(LocationStatus::Ready);
    set_following(true);
    for k in 1..=10 {
        let heading = 1.6 * f64::from(k);
        sensor::set_heading(Some(heading));
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
