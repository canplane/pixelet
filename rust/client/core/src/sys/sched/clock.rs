// pxl-core/sys/sched/clock.rs

//! The wall clock time, which each frame from the runtime carries and the
//! door sets as it takes the frame. The world's time of day reads it.

use xpute_core::global::Global;

static WALL_MS: Global<f64> = Global::new();

pub fn set_wall_now(unix_ms: f64) {
    WALL_MS.set(unix_ms);
}

/// The wall clock time in Unix ms, as of the last frame. It reads 0 before
/// the first frame.
pub fn wall_now() -> f64 {
    *WALL_MS.get(|| 0.0)
}
