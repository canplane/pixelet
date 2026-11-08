// pxl-core/sys/sched/edge.rs

//! The world's turn edge: when its tasks run, and a frame asked for.

use xpute_core::clock;
use xpute_core::global::Global;
use xpute_core::sched::edge::Edge;
use xpute_core::sched::task;

use crate::sys::sched::pacing_spec::MARGIN_SHARE;
use crate::sys::sched::param::CONTENT_SHARE;

static EDGE: Global<Edge> = Global::new();

pub fn world_edge() -> &'static mut Edge {
    EDGE.get(Edge::new)
}

/// The turn's tasks keep their share past the frame that ran before them,
/// `CONTENT_SHARE` of the frame the runtime paces less its margin, as the
/// deadline takes it, however far the frame overran.
pub fn tasks_after_frame(paced_ms: f64) {
    world_edge().extend_to(clock::now() + CONTENT_SHARE * paced_ms * (1.0 - MARGIN_SHARE));
}

/// A task queued and not yet polled (a turn without a frame polls none)
/// asks for the next turn too.
pub fn world_fall() -> f64 {
    if task::pending() {
        world_edge().ask_next();
    }
    world_edge().fall()
}

pub fn request_world_tick() {
    world_edge().ask_next();
}

/// The earliest asked wins; a wait still pending when its turn comes asks
/// again.
pub fn wake_at(at: f64) {
    world_edge().wake_at(at);
}
