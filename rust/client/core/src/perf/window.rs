// pxl-core/perf/window.rs

//! The profiler's window turns recording, and the tick's step timing that
//! the report reads, on and off together.

use crate::perf::record::{perf_recording_start, perf_recording_stop, perf_release};
use crate::sys::sched::tick::tick;

/// Follows the profiler's setting. It is called with the stored setting on
/// the turn the settings are read, a few turns after boot, so a profiler left
/// on has the world's start in its window. When the setting is off, the
/// recording's columns go back to the heap.
pub fn perf_follow(on: bool) {
    if on {
        perf_window_start();
    } else {
        perf_window_stop();
        perf_release();
    }
}

/// Returns false, and records nothing, when the heap refuses the
/// recording's columns.
pub fn perf_window_start() -> bool {
    if !perf_recording_start() {
        return false;
    }
    tick().step_stats_reset();
    tick().set_step_timing(true);
    true
}

pub fn perf_window_stop() {
    perf_recording_stop();
    tick().set_step_timing(false);
}
