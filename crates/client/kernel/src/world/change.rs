// pxl-kernel/world/change.rs

//! The world's turn edge: when its tasks run, and a frame asked for, since
//! the canvas renders on demand.

use xpute_runtime::sched::edge::Edge;

use xpute_runtime::global::Global;

/// The kernel's tasks (`world_tasks`); an executor sizes its pool by it.
pub const TASKS: usize = 6;

static EDGE: Global<Edge<TASKS>> = Global::new();

pub fn world_edge() -> &'static mut Edge<TASKS> {
    EDGE.get(Edge::new)
}

/// A task queued and not yet polled (a turn without a frame polls none)
/// asks for the next turn too.
pub fn world_fall() -> f64 {
    if xpute_runtime::sched::task::pending() {
        world_edge().ask_next();
    }
    world_edge().fall()
}

pub fn request_world_tick() {
    world_edge().ask_next();
}

/// The earliest asked wins; a wait still pending when its turn comes asks again.
pub fn wake_at(at: f64) {
    world_edge().wake_at(at);
}
