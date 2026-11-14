// pxl-core/domain/scene/rate.test.rs

use super::*;
use crate::domain::setting::settings_mut;

/// A 120 Hz display's interval, in ms.
const DISPLAY_MS: f64 = 1000.0 / 120.0;

/// A drawn frame more than half an interval later than its target adds an
/// interval, up to `RATE_MIN_HZ`'s; one whose cost fits one interval fewer by
/// `RES_BAND` takes one off; and any setting but `auto` draws at every vsync.
#[test]
fn the_rate_steps_an_interval_at_a_time() {
    settings_mut().res = ResMode::Auto;
    judge(1.5 * DISPLAY_MS, 0.0, DISPLAY_MS);
    assert_eq!(interval(), 1, "half an interval late");
    judge(1.6 * DISPLAY_MS, 0.0, DISPLAY_MS);
    assert_eq!(interval(), 2);
    for _ in 0..10 {
        judge(100.0 * DISPLAY_MS, 0.0, DISPLAY_MS);
    }
    let slowest = (1000.0 / param::RATE_MIN_HZ / DISPLAY_MS).round_ties_even() as u32;
    assert_eq!(interval(), slowest, "never slower than RATE_MIN_HZ");
    let edge_ms = f64::from(slowest - 1) * DISPLAY_MS * (1.0 - param::RES_BAND);
    judge(f64::from(slowest) * DISPLAY_MS, edge_ms, DISPLAY_MS);
    assert_eq!(interval(), slowest, "no room by the band");
    judge(f64::from(slowest) * DISPLAY_MS, 0.99 * edge_ms, DISPLAY_MS);
    assert_eq!(interval(), slowest - 1);
    settings_mut().res = ResMode::Fixed;
    judge(100.0 * DISPLAY_MS, 0.0, DISPLAY_MS);
    assert_eq!(interval(), 1);
}
