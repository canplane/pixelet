// pxl-core/domain/scene/rate.rs

//! The frame's target interval (`target_ms`), a whole number of the display's
//! measured intervals (sys/sched/display.rs), which the frame task keeps by
//! skipping the vsyncs that come before it is due (`is_due`, frame.rs
//! `frame_task`), both as the display is measured now (`pace_ms`). Before
//! the display is measured, its interval is taken to be one interval at
//! `RATE_MAX_HZ` (`display_ms`). The target is never
//! shorter than the most display intervals that fit one interval at
//! `RATE_MAX_HZ`, at least one, and never longer than `RATE_MIN_HZ` allows.
//! Outside `auto` it is the shortest.
//! In `auto` it is the frame rate's own: it judges moving frames only, and not
//! the first after the view stood still, as resolution.rs does
//! (`Motion::Start`). A judged frame that came more than half an interval
//! later than its target adds an interval; one whose cost, the GPU's busy time
//! where a read timed it and the frame task's own, would fit one interval
//! fewer by `RES_BAND` takes one off.

use pxl_global::Global;

use crate::domain::scene::param;
use crate::domain::setting::{ResMode, settings};
use crate::sys::sched::display;

/// How a frame was paced.
#[derive(Clone, Copy)]
pub(crate) struct Pace {
    /// The real time since the last frame, in ms.
    pub gap_ms: f64,
    /// The display's interval, in ms, as `display_ms` takes it.
    pub display_ms: f64,
    pub motion: Motion,
}

/// Whether a frame moved: whether it asked for the next (sys/sched.rs
/// `invalidate`), and whether the last frame did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    /// The frame asked for no next.
    Still,
    /// It moved and the last frame did not. A judge counts moving frames
    /// only, and not this one: its gap is how long the view stood still.
    Start,
    /// It moved, and so did the last.
    Moving,
}

/// `auto`'s target interval, in display intervals, as last judged.
static INTERVAL: Global<u32> = Global::new();

fn interval_mut() -> &'static mut u32 {
    INTERVAL.get(|| 1)
}

/// The fewest display intervals of `display_ms` a frame takes, its interval's
/// floor: the most that
/// fit one interval at `RATE_MAX_HZ`, at least one. An eighth of an interval
/// is allowed for the spread of the display's measure, so a 120 Hz display
/// measured at 8.34 ms still takes two.
fn interval_floor(display_ms: f64) -> u32 {
    ((1000.0 / param::RATE_MAX_HZ + display_ms / 8.0) / display_ms)
        .floor()
        .max(1.0) as u32
}

/// The most display intervals of `display_ms` a frame takes, its interval's
/// cap: the nearest to
/// one interval at `RATE_MIN_HZ`, at least one.
fn interval_cap(display_ms: f64) -> u32 {
    ((1000.0 / param::RATE_MIN_HZ) / display_ms + 0.5)
        .floor()
        .max(1.0) as u32
}

/// The display intervals of `display_ms` the next frame's target spans.
fn interval(display_ms: f64) -> u32 {
    let floor = interval_floor(display_ms);
    match settings().res {
        ResMode::Auto => (*interval_mut()).clamp(floor, interval_cap(display_ms)),
        _ => floor,
    }
}

/// The display's interval and the frame's target interval, in ms, as the
/// display is measured now (sys/sched/display.rs).
pub(crate) fn pace_ms() -> (f64, f64) {
    let display_ms = display_ms(display::interval_ms());
    (display_ms, target_ms(display_ms))
}

/// The display's interval, in ms, as the frame's pacing takes it: the
/// interval `measured`, or before the display is measured (0), one interval
/// at `RATE_MAX_HZ`.
pub(crate) fn display_ms(measured: f64) -> f64 {
    if measured > 0.0 {
        measured
    } else {
        1000.0 / param::RATE_MAX_HZ
    }
}

/// The frame's target interval, in ms, on a display whose interval is
/// `display_ms` (`display_ms`).
pub(crate) fn target_ms(display_ms: f64) -> f64 {
    f64::from(interval(display_ms)) * display_ms
}

/// Whether a vsync `since_ms` after the last frame's start opens the next
/// frame, paced as `pace_ms` answers, the display's interval and the target:
/// once the target has passed, less half a display interval for the vsync's
/// own jitter. A vsync that does not is skipped.
pub(crate) fn is_due(since_ms: f64, (display_ms, target_ms): (f64, f64)) -> bool {
    since_ms >= target_ms - display_ms / 2.0
}

/// Judges a drawn frame paced as `pace` that cost `cost_ms`, in `auto` alone.
pub(crate) fn judge(pace: Pace, cost_ms: f64) {
    let (gap_ms, display_ms) = (pace.gap_ms, pace.display_ms);
    if settings().res != ResMode::Auto || pace.motion != Motion::Moving {
        return;
    }
    let (floor, n) = (interval_floor(display_ms), interval(display_ms));
    let next = if gap_ms > (f64::from(n) + 0.5) * display_ms {
        n + 1
    } else if n > floor && cost_ms < f64::from(n - 1) * display_ms * (1.0 - param::RES_BAND) {
        n - 1
    } else {
        n
    };
    *interval_mut() = next.clamp(floor, interval_cap(display_ms));
}

#[cfg(test)]
#[path = "rate.test.rs"]
mod test;
