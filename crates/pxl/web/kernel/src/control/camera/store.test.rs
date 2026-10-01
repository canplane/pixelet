// pxl-kernel/control/camera/store.test.rs
//
// The tilt goes from straight down to straight up and no further: a drag
// past either end holds there.

use super::*;
use crate::global::serial;

#[test]
fn the_tilt_stops_at_straight_down_and_straight_up() {
    let _g = serial();
    let up = core::f64::consts::PI;
    for (asked, held) in [(10.0, up), (-10.0, 0.0), (2.0, 2.0), (-2.0, 0.0), (up, up), (0.0, 0.0)] {
        let merged = camera_store().merged(IntentPatch {
            polar: Some(asked),
            ..Default::default()
        });
        assert_eq!(merged.polar, held, "asked {asked}");
    }
}
