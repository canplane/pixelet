// pxl-core/perf/window.rs

//! The profiler's window turns recording, and the tick's step timing that
//! the report reads, on and off together.

use crate::perf::record::{
    perf_recording_start, perf_recording_stop, perf_release, set_step_timing,
};

/// Follows the profiler's setting. It is called with the stored setting on
/// the frame the settings are read, a few frames after boot, so a profiler left
/// on has the world's start in its window. When the setting is off, the
/// recording's tables go back to the heap.
pub(crate) fn perf_follow(on: bool) {
    if on {
        perf_window_start();
    } else {
        perf_window_stop();
        perf_release();
    }
}

/// Returns false, and records nothing, when the heap refuses the
/// recording's tables.
pub(crate) fn perf_window_start() -> bool {
    if !perf_recording_start() {
        return false;
    }
    set_step_timing(true);
    true
}

pub(crate) fn perf_window_stop() {
    perf_recording_stop();
    set_step_timing(false);
}
