// pxl-kernel/profiler/window.rs

//! The profiler's window: recording, and the steps' timing the report reads,
//! on and off together.

use crate::profiler::record::{profiler_recording_start, profiler_recording_stop};
use crate::sched::tick::{set_step_timing, tick_step_stats_reset};

/// Called at WORLD_BOOT with the stored setting, before the first turn, so
/// a profiler left on has the boot in its window.
pub fn profiler_follow(on: bool) {
    if on {
        profiler_window_start();
    } else {
        profiler_window_stop();
    }
}

/// Step timing starts here, not at boot: it is a clock read around every
/// step of every tick, and only the report reads it.
pub fn profiler_window_start() {
    profiler_recording_start();
    tick_step_stats_reset();
    set_step_timing(true);
}

pub fn profiler_window_stop() {
    profiler_recording_stop();
    set_step_timing(false);
}
