// pxl-kernel/sys/sched/edge.rs

//! The world's turn edge: when its tasks run, and a frame asked for, since
//! the canvas renders on demand.

use xpute_guest::sched::edge::Edge;

use xpute_guest::global::Global;

/// The kernel's tasks: the three that run for good (`world_tasks`) and one for
/// each file in flight, a slot's, the catalog's or the sphere's (sys/io/loader.rs
/// `arrival`); an executor sizes its pool by it.
pub const TASKS: usize = 3 + crate::sys::load::LOAD_FILES;

/// The order the kernel's tasks ask of the executor, which may ignore it;
/// nothing relies on it.
#[repr(u8)]
#[derive(Clone, Copy)]
pub(crate) enum TaskPriority {
    Frame,
    Arrival,
    Background,
}

static EDGE: Global<Edge<TASKS>> = Global::new();

pub fn world_edge() -> &'static mut Edge<TASKS> {
    EDGE.get(Edge::new)
}

/// A task queued and not yet polled (a turn without a frame polls none)
/// asks for the next turn too.
pub fn world_fall() -> f64 {
    if xpute_guest::sched::task::pending() {
        world_edge().ask_next();
    }
    world_edge().fall()
}

pub fn request_world_tick() {
    world_edge().ask_next();
}

/// The turn's tasks keep their share past the frame that ran before them
/// (`param::CONTENT_SHARE` of the frame the host paces, the margin taken as
/// the quota takes it), whatever the frame overran: what the frame owes the
/// host comes off its own next quota, never the tasks' turn.
pub fn tasks_after_frame(paced_ms: f64) {
    use crate::sys::sched::param::{CONTENT_SHARE, QUANTUM_MARGIN_SHARE};
    world_edge().extend_to(xpute_guest::clock::now() + CONTENT_SHARE * paced_ms * (1.0 - QUANTUM_MARGIN_SHARE));
}

/// The earliest asked wins; a wait still pending when its turn comes asks again.
pub fn wake_at(at: f64) {
    world_edge().wake_at(at);
}
