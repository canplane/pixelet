// pxl-kernel/sys/sched/edge.rs

//! The world's turn edge: when its tasks run, and a frame asked for.

use xpute_core::sched::edge::Edge;

use xpute_core::clock;
use xpute_core::global::Global;
use xpute_core::sched::task;

/// The most tasks the world's edge parks at once, a task parking at most
/// once a turn, with room to spare; the kernel runs no more tasks than it.
pub const TASKS_MOST: usize = 256;

/// The order the kernel's tasks ask of the executor, which may ignore it.
#[repr(u8)]
#[derive(Clone, Copy)]
pub(crate) enum TaskPriority {
    Frame,
    Arrival,
}

static EDGE: Global<Edge<TASKS_MOST>> = Global::new();

pub fn world_edge() -> &'static mut Edge<TASKS_MOST> {
    EDGE.get(Edge::new)
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

/// The turn's tasks keep their share past the frame that ran before them
/// (`param::CONTENT_SHARE` of the frame the runtime paces, the margin taken as
/// the deadline takes it), whatever the frame overran.
pub fn tasks_after_frame(paced_ms: f64) {
    use crate::sys::sched::pacing_spec::MARGIN_SHARE;
    use crate::sys::sched::param::CONTENT_SHARE;
    world_edge().extend_to(clock::now() + CONTENT_SHARE * paced_ms * (1.0 - MARGIN_SHARE));
}

/// The earliest asked wins; a wait still pending when its turn comes asks again.
pub fn wake_at(at: f64) {
    world_edge().wake_at(at);
}
