// pxl-core/domain/scene/rate.rs

//! `auto`'s frame rate: a frame every `interval` of the display's measured
//! intervals (sys/sched/display.rs), which the frame task keeps by skipping
//! vsyncs (frame.rs `frame_task`). It judges moving frames only, and not the
//! first after the view stood still, as resolution.rs does
//! (`Motion::Start`). A
//! judged frame that came more than half an interval later than its target
//! adds an interval; one whose cost, the GPU's busy time where a read timed it
//! and the frame task's own, would fit one interval fewer by `RES_BAND` takes
//! one off. It never paces slower than `RATE_MIN_HZ`. Any other setting draws
//! at every vsync.

use pxl_global::Global;

use crate::domain::scene::param;
use crate::domain::setting::{ResMode, settings};

/// How a frame was paced.
#[derive(Clone, Copy)]
pub(crate) struct Pace {
    /// The real time since the last frame, in ms.
    pub gap_ms: f64,
    /// The display's interval, in ms (sys/sched/display.rs).
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

/// The frame's target interval, in display intervals.
static INTERVAL: Global<u32> = Global::new();

fn interval_mut() -> &'static mut u32 {
    INTERVAL.get(|| 1)
}

pub(crate) fn interval() -> u32 {
    *interval_mut()
}

/// Judges a drawn frame paced as `pace` that cost `cost_ms`.
pub(crate) fn judge(pace: Pace, cost_ms: f64) {
    let i = interval_mut();
    let (gap_ms, display_ms) = (pace.gap_ms, pace.display_ms);
    if settings().res != ResMode::Auto || display_ms <= 0.0 {
        *i = 1;
        return;
    }
    if pace.motion != Motion::Moving {
        return;
    }
    let n = *i;
    let slowest = ((1000.0 / param::RATE_MIN_HZ) / display_ms + 0.5)
        .floor()
        .max(1.0) as u32;
    let next = if gap_ms > (f64::from(n) + 0.5) * display_ms {
        n + 1
    } else if n > 1 && cost_ms < f64::from(n - 1) * display_ms * (1.0 - param::RES_BAND) {
        n - 1
    } else {
        n
    };
    *i = next.min(slowest);
}

#[cfg(test)]
#[path = "rate.test.rs"]
mod test;
