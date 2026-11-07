// pxl-core/perf/window.rs

//! The profiler's window: recording, and the steps' timing the report reads,
//! on and off together.

use crate::perf::record::{perf_recording_start, perf_recording_stop, perf_release};
use crate::sys::sched::tick::{set_step_timing, tick_step_stats_reset};

/// Called with the stored setting the turn the settings are read, a few
/// turns from boot, so a profiler left on has the world's start in its
/// window; turned off, the recording's columns go back to the heap.
pub fn perf_follow(on: bool) {
    if on {
        perf_window_start();
    } else {
        perf_window_stop();
        perf_release();
    }
}

/// False, nothing recorded, where the heap refuses the recording's columns.
pub fn perf_window_start() -> bool {
    if !perf_recording_start() {
        return false;
    }
    tick_step_stats_reset();
    set_step_timing(true);
    true
}

pub fn perf_window_stop() {
    perf_recording_stop();
    set_step_timing(false);
}
