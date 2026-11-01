// pxl-kernel/domain/scene/environment/cloud.test.rs

use super::*;
use crate::domain::view::lens;
use crate::sys;

/// The drift's step as it was written before the view held the seen step:
/// a twelfth of a cell at the nearest drawn, over the wind's speed.
fn drift_step_s_as_written(seen: f64) -> f64 {
    1.0 / 12.0 * lens::cell_there(seen) / libm::hypot(WIND[0], WIND[1])
}

#[test]
fn cloud_steps_unchanged() {
    let _g = sys::serial();
    let was = clouds().seen;
    for seen in [0.5, 3.0, 47.25, 1.0e3, 6.4e4, 3.1e6, f64::INFINITY] {
        clouds().seen = seen;
        assert_eq!(
            drift_step_s().to_bits(),
            drift_step_s_as_written(seen).to_bits(),
            "at {seen}"
        );
    }
    clouds().seen = was;
}
