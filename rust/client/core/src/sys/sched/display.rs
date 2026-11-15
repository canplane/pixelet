// pxl-core/sys/sched/display.rs

//! The display's interval, measured from pxl-gpu-guest's vsync answers: the gaps
//! between vsyncs armed back to back, read at their low quartile, so a late
//! answer and the early one after it do not move it.

use pxl_global::Global;

/// How many gaps the interval is read from.
const GAP_CAP: usize = 16;

#[derive(Default)]
struct Display {
    /// The last vsync's answer, while vsyncs are armed back to back.
    at: Option<f64>,
    /// The last gaps, oldest first once the ring has wrapped, and how many
    /// were seen.
    ring: [f64; GAP_CAP],
    seen_cnt: usize,
}

static DISPLAY: Global<Display> = Global::new();

fn display() -> &'static mut Display {
    DISPLAY.get(Display::default)
}

/// A vsync answered at `at`.
pub(crate) fn answered(at: f64) {
    let d = display();
    if let Some(last) = d.at {
        d.ring[d.seen_cnt % GAP_CAP] = at - last;
        d.seen_cnt += 1;
    }
    d.at = Some(at);
}

/// The next vsync is not armed back to back with the last.
pub(crate) fn idle() {
    display().at = None;
}

/// The display's interval in ms, or 0 before two vsyncs came back to back.
pub(crate) fn interval_ms() -> f64 {
    let d = display();
    let n = d.seen_cnt.min(GAP_CAP);
    if n == 0 {
        return 0.0;
    }
    let mut gaps = d.ring;
    let gaps = &mut gaps[..n];
    gaps.sort_unstable_by(f64::total_cmp);
    gaps[n / 4]
}
