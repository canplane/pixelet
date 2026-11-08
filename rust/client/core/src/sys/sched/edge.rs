// pxl-core/sys/sched/edge.rs

//! The one turn edge that every task of the core runs against
//! (xpute_core::sched::edge). It decides when the turn's tasks yield and
//! whether the core asks for the next turn.

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

/// Lets the turn's tasks run for their share of the turn after the frame,
/// however far the frame overran the deadline. The share is `CONTENT_SHARE`
/// of the frame time the runtime paces, less the runtime's margin, which the
/// deadline leaves out as well.
pub fn tasks_after_frame(paced_ms: f64) {
    world_edge().extend_to(clock::now() + CONTENT_SHARE * paced_ms * (1.0 - MARGIN_SHARE));
}

/// The ms until the core wants its next turn (`Edge::fall`). A task that is
/// queued and not yet polled asks for the next turn too, because a turn with
/// no frame and no deadline left polls no task.
pub fn world_fall() -> f64 {
    if task::pending() {
        world_edge().ask_next();
    }
    world_edge().fall()
}

pub fn request_world_tick() {
    world_edge().ask_next();
}

/// Asks for a turn at `at`. Of several asks the earliest wins, and a wait
/// that is still pending when its turn comes must ask again.
pub fn wake_at(at: f64) {
    world_edge().wake_at(at);
}
