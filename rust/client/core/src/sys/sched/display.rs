// pxl-core/sys/sched/display.rs

//! The display's interval, read from the gaps pxl-gpu-guest's vsync answers:
//! each the time since the last frame the host saw before it, one frame's
//! but across a pause (spec/gpu/wit/vsync.wit), read at their low quartile,
//! so a gap across a pause or a frame the display dropped does not move it
//! while more than a quarter of the gaps are one.

use pxl_global::Global;

/// How many gaps the interval is read from.
const GAP_CAP: usize = 16;

#[derive(Default)]
struct Display {
    /// The last gaps, oldest first once the ring has wrapped, and how many
    /// were seen.
    ring: [f64; GAP_CAP],
    seen_cnt: usize,
}

static DISPLAY: Global<Display> = Global::new();

fn display() -> &'static mut Display {
    DISPLAY.get(Display::default)
}

/// A vsync answered a frame `gap_ms` after the last the host saw, 0 for the
/// host's first.
pub(crate) fn answered(gap_ms: f64) {
    if gap_ms <= 0.0 {
        return;
    }
    let d = display();
    d.ring[d.seen_cnt % GAP_CAP] = gap_ms;
    d.seen_cnt += 1;
}

/// The display's interval in ms, or 0 before a vsync answered a gap.
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
