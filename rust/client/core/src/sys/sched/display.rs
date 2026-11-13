// pxl-core/sys/sched/display.rs

//! The display's interval, measured from xpute-gpu-guest's vsync answers: the gaps
//! between vsyncs armed back to back, read at their low quartile, so a late
//! answer and the early one after it do not move it.

use std::cell::Cell;

/// How many gaps the interval is read from.
const GAPS: usize = 16;

thread_local! {
    /// The last vsync's answer, while vsyncs are armed back to back.
    static AT: Cell<Option<f64>> = const { Cell::new(None) };
    /// The last gaps, oldest first once the ring has wrapped, and how many
    /// were seen.
    static RING: Cell<[f64; GAPS]> = const { Cell::new([0.0; GAPS]) };
    static SEEN: Cell<usize> = const { Cell::new(0) };
}

/// A vsync answered at `at`.
pub fn answered(at: f64) {
    if let Some(last) = AT.get() {
        let mut ring = RING.get();
        let seen = SEEN.get();
        ring[seen % GAPS] = at - last;
        RING.set(ring);
        SEEN.set(seen + 1);
    }
    AT.set(Some(at));
}

/// The next vsync is not armed back to back with the last.
pub fn idle() {
    AT.set(None);
}

/// The display's interval in ms, or 0 before two vsyncs came back to back.
pub fn interval_ms() -> f64 {
    let n = SEEN.get().min(GAPS);
    if n == 0 {
        return 0.0;
    }
    let mut gaps = RING.get();
    let gaps = &mut gaps[..n];
    gaps.sort_unstable_by(f64::total_cmp);
    gaps[n / 4]
}
