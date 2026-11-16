// pxl-core/domain/scene/rate.test.rs

use super::*;
use crate::domain::setting::settings_mut;
use crate::sys::sched::display;

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

/// The target on that display, in its intervals.
fn target() -> f64 {
    (target_ms(DISPLAY_MS) / DISPLAY_MS).round_ties_even()
}

/// Outside `auto` a frame takes the most display intervals that fit one
/// interval at `RATE_MAX_HZ`, at least one, however late or costly frames
/// come: two at 120 Hz, two at 144 Hz, one at 90 Hz and at 60 Hz. Before the
/// display is measured, the target is one interval at `RATE_MAX_HZ`.
#[test]
fn the_target_is_paced_to_the_cap() {
    settings_mut().res = ResMode::Fixed;
    judge(pace(100.0 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(target(), 2.0, "120 Hz");
    for (hz, n) in [(144.0, 2.0), (90.0, 1.0), (60.0, 1.0)] {
        assert_eq!(target_ms(1000.0 / hz), n * 1000.0 / hz, "{hz} Hz");
    }
    // A 120 Hz display measured a little long still takes two.
    assert_eq!(target_ms(8.34), 2.0 * 8.34);
    assert_eq!(
        target_ms(display_ms(0.0)),
        1000.0 / param::RATE_MAX_HZ,
        "not measured"
    );
}

/// Paces `n` vsyncs `gap_ms` apart, each with the next of `jitter_ms` added,
/// as the frame task does: each measures the display, and one that is due
/// opens a frame. Answers which vsyncs opened one.
fn paced(gap_ms: f64, jitter_ms: &[f64], n: i32) -> Vec<bool> {
    let mut frame_at = f64::NEG_INFINITY;
    (0..n)
        .zip(jitter_ms.iter().cycle())
        .map(|(k, jitter)| {
            let at = f64::from(k) * gap_ms + jitter;
            display::answered(at);
            let due = is_due(at - frame_at, pace_ms());
            if due {
                frame_at = at;
            }
            due
        })
        .collect()
}

/// A 120 Hz display not yet measured takes one interval at `RATE_MAX_HZ` as
/// its own, so its first vsync opens a frame; the next lands the first gap,
/// one interval, and from then on a frame opens at every other vsync.
#[test]
fn a_120_hz_display_paces_every_other_vsync_once_measured() {
    settings_mut().res = ResMode::Fixed;
    let opened = paced(DISPLAY_MS, &[0.0], 12);
    assert_eq!(opened[..2], [true, false], "the first gap lands");
    assert!((display::interval_ms() - DISPLAY_MS).abs() < 1e-9);
    let every_other: Vec<bool> = (0..12).map(|k| k % 2 == 0).collect();
    assert_eq!(opened, every_other);
}

/// A 60 Hz display opens a frame at every vsync, its jitter skipping none.
#[test]
fn a_60_hz_display_keeps_every_vsync() {
    settings_mut().res = ResMode::Fixed;
    let opened = paced(1000.0 / 60.0, &[0.0, 0.3, -0.3, 0.1, -0.2], 20);
    assert!(opened.iter().all(|&o| o));
}

/// Gaps across frames that ran past a vsync are whole numbers of intervals,
/// and the low quartile reads past them while more than a quarter of the
/// gaps are one interval: five of sixteen here.
#[test]
fn the_low_quartile_reads_past_long_gaps() {
    let mut at = 0.0;
    display::answered(at);
    for n in [
        2.0, 3.0, 1.0, 2.0, 3.0, 1.0, 2.0, 1.0, 3.0, 2.0, 1.0, 2.0, 3.0, 1.0, 3.0, 2.0,
    ] {
        at += n * DISPLAY_MS;
        display::answered(at);
    }
    assert!((display::interval_ms() - DISPLAY_MS).abs() < 1e-9);
}

/// In `auto`, a drawn frame more than half an interval later than its target
/// adds an interval, up to `RATE_MIN_HZ`'s; one whose cost fits one interval
/// fewer by `RES_BAND` takes one off, down to the floor's.
#[test]
fn the_rate_steps_an_interval_at_a_time() {
    settings_mut().res = ResMode::Auto;
    assert_eq!(target(), 2.0, "the floor's at first");
    judge(pace(2.5 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(target(), 2.0, "half an interval late");
    judge(pace(2.6 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(target(), 3.0);
    for _ in 0..10 {
        judge(pace(100.0 * DISPLAY_MS, Motion::Moving), 0.0);
    }
    let cap = (1000.0 / param::RATE_MIN_HZ / DISPLAY_MS).round_ties_even();
    assert_eq!(target(), cap, "never slower than RATE_MIN_HZ");
    let edge_ms = (cap - 1.0) * DISPLAY_MS * (1.0 - param::RES_BAND);
    judge(pace(cap * DISPLAY_MS, Motion::Moving), edge_ms);
    assert_eq!(target(), cap, "no room by the band");
    judge(pace(cap * DISPLAY_MS, Motion::Moving), 0.99 * edge_ms);
    assert_eq!(target(), cap - 1.0);
    for _ in 0..10 {
        judge(pace(2.0 * DISPLAY_MS, Motion::Moving), 0.0);
    }
    assert_eq!(target(), 2.0, "never faster than RATE_MAX_HZ");
}

/// Only moving frames are judged, and not the first after the view stood
/// still: its gap is how long the view stood, not how late it came. A frame
/// that starts motion after a pause, or a still one, leaves the interval as it
/// was.
#[test]
fn a_still_spell_is_no_late_frame() {
    settings_mut().res = ResMode::Auto;
    judge(pace(100.0 * DISPLAY_MS, Motion::Still), 0.0);
    assert_eq!(target(), 2.0, "a still frame is not judged");
    judge(pace(500.0 * DISPLAY_MS, Motion::Start), 0.0);
    assert_eq!(target(), 2.0, "the frame that starts motion is not judged");
    judge(pace(2.6 * DISPLAY_MS, Motion::Moving), 0.0);
    assert_eq!(target(), 3.0, "a late moving frame is");
}
