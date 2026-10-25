// pxl-kernel/profiler/window.rs

//! The profiler's window: recording, and the steps' timing the report reads,
//! on and off together.

use crate::profiler::record::{profiler_recording_start, profiler_recording_stop, profiler_release};
use crate::sys::sched::tick::{set_step_timing, tick_step_stats_reset};

/// Called with the stored setting the turn the settings are read, a few
/// turns from boot, so a profiler left on has the world's start in its
/// window; turned off, the recording's lanes go back to the heap.
pub fn profiler_follow(on: bool) {
    if on {
        profiler_window_start();
    } else {
        profiler_window_stop();
        profiler_release();
    }
}

/// Step timing starts here, not at boot: it is a clock read around every
/// step of every tick, and only the report reads it. False, nothing
/// recorded, where the heap refuses the recording's lanes.
pub fn profiler_window_start() -> bool {
    if !profiler_recording_start() {
        return false;
    }
    tick_step_stats_reset();
    set_step_timing(true);
    true
}

pub fn profiler_window_stop() {
    profiler_recording_stop();
    set_step_timing(false);
}
