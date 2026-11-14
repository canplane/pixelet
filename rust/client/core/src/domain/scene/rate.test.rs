// pxl-core/domain/scene/rate.test.rs

use super::*;
use crate::domain::setting::settings_mut;

/// A 120 Hz display's interval, in ms.
const DISPLAY_MS: f64 = 1000.0 / 120.0;

/// A frame `gap_ms` after the last on that display, in `motion`.
fn pace(gap_ms: f64, motion: Motion) -> Pace {
    Pace {
        gap_ms,
        display_ms: DISPLAY_MS,
        motion,
    }
}

/// A drawn frame more than half an interval later than its target adds an
/// interval, up to `RATE_MIN_HZ`'s; one whose cost fits one interval fewer by
/// `RES_BAND` takes one off; and any setting but `auto` draws at every vsync.
#[test]
fn the_rate_steps_an_interval_at_a_time() {
    settings_mut().res = ResMode::Auto;
    judge(pace(1.5 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(interval(), 1, "half an interval late");
    judge(pace(1.6 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(interval(), 2);
    for _ in 0..10 {
        judge(pace(100.0 * DISPLAY_MS, Motion::Moving), 0.0);
    }
    let slowest = (1000.0 / param::RATE_MIN_HZ / DISPLAY_MS).round_ties_even() as u32;
    assert_eq!(interval(), slowest, "never slower than RATE_MIN_HZ");
    let edge_ms = f64::from(slowest - 1) * DISPLAY_MS * (1.0 - param::RES_BAND);
    judge(
        pace(f64::from(slowest) * DISPLAY_MS, Motion::Moving),
        edge_ms,
    );
    assert_eq!(interval(), slowest, "no room by the band");
    judge(
        pace(f64::from(slowest) * DISPLAY_MS, Motion::Moving),
        0.99 * edge_ms,
    );
    assert_eq!(interval(), slowest - 1);
    settings_mut().res = ResMode::Fixed;
    judge(pace(100.0 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(interval(), 1);
}

/// Only moving frames are judged, and not the first after the view stood
/// still: its gap is how long the view stood, not how late it came. A frame
/// that starts motion after a pause, or a still one, leaves the interval as it
/// was.
#[test]
fn a_still_spell_is_no_late_frame() {
    settings_mut().res = ResMode::Auto;
    judge(pace(100.0 * DISPLAY_MS, Motion::Still), 0.0);
    assert_eq!(interval(), 1, "a still frame is not judged");
    judge(pace(500.0 * DISPLAY_MS, Motion::Start), 0.0);
    assert_eq!(interval(), 1, "the frame that starts motion is not judged");
    judge(pace(1.6 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(interval(), 2, "a late moving frame is");
}
