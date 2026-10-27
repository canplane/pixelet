// pxl-kernel/sys/sched/clock.rs

//! The wall clock, as each frame the host hands tells it: what the world's
//! time of day reads, set where the door takes the frame.

use xpute_guest::global::Global;

static WALL_MS: Global<f64> = Global::new();

/// The wall clock as the last frame told it, in Unix ms; 0 before the first.
pub fn wall_now() -> f64 {
    *WALL_MS.get(|| 0.0)
}

pub fn set_wall_now(unix_ms: f64) {
    WALL_MS.set(unix_ms);
}
