// pxl-core/sys/sched/clock.rs

//! The wall clock, as each frame the runtime hands tells it, set where the
//! door takes the frame: what the world's time of day reads.

use xpute_core::global::Global;

static WALL_MS: Global<f64> = Global::new();

pub fn set_wall_now(unix_ms: f64) {
    WALL_MS.set(unix_ms);
}

/// In Unix ms, as the last frame told it; 0 before the first.
pub fn wall_now() -> f64 {
    *WALL_MS.get(|| 0.0)
}
