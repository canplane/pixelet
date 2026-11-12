// pxl-core/domain/scene/rate.rs

//! `auto`'s frame rate: a frame every `intervals` of the display's measured
//! intervals (sys/sched/display.rs), which the frame task keeps by skipping
//! vsyncs (frame.rs `frame_task`). A drawn frame that came more than half an
//! interval later than its target adds an interval; one whose cost, the GPU's
//! busy time where a read timed it and the frame task's own, would fit one
//! interval fewer by `RES_BAND` takes one off. It never paces slower than
//! `RATE_MIN_HZ`. Any other setting draws at every vsync.

use std::cell::Cell;

use crate::domain::scene::param;
use crate::domain::setting::store::{settings, ResMode};

thread_local! {
    /// The frame's target interval, in display intervals.
    static INTERVALS: Cell<u32> = const { Cell::new(1) };
}

pub fn intervals() -> u32 {
    INTERVALS.get()
}

/// Judges a drawn frame that came `gap_ms` after the last and cost
/// `cost_ms`, on a display of `display_ms`.
pub fn judge(gap_ms: f64, cost_ms: f64, display_ms: f64) {
    if settings().res != ResMode::auto || display_ms <= 0.0 {
        INTERVALS.set(1);
        return;
    }
    let n = INTERVALS.get();
    let slowest = ((1000.0 / param::RATE_MIN_HZ) / display_ms + 0.5).floor().max(1.0) as u32;
    let next = if gap_ms > (f64::from(n) + 0.5) * display_ms {
        n + 1
    } else if n > 1 && cost_ms < f64::from(n - 1) * display_ms * (1.0 - param::RES_BAND) {
        n - 1
    } else {
        n
    };
    INTERVALS.set(next.min(slowest));
}
