// pxl-kernel/world/change.rs

//! The world's change log: which units of world-shared state changed, read by
//! whoever cares, the way issue/20260913-the-read-direction-a-change-log.md
//! describes. The log is the kernel's (xpute_runtime::ipc::change), where the
//! state that changes most — the object store — bumps it itself; this is its
//! face for the modules: the topics, a bump, and the frame a bump asks for.
//!
//! A source calls `bump_world`. Readers compare revisions or walk the
//! journal. The canvas renders on demand, so a bump also asks for that frame;
//! the request goes back to the host on the turn's falling edge.

use xpute_runtime::ipc::change::{ChangeLog, Revision, Topic};

use xpute_runtime::sched::edge::Edge;

use crate::global::Global;
use crate::kernel::kernel;

/// A column's cube content (object/store.rs), keyed by column base.
pub const OBJECT_COLUMN: Topic = 0;
/// A column's placed decor, apart from its cubes, keyed by column base.
pub const DECOR_COLUMN: Topic = 1;
/// One cell's placed entries, keyed by cell corner.
pub const OBJECT_CELL: Topic = 2;
/// A resident terrain page arrived or left, keyed by page index.
pub const TERRAIN_PAGE: Topic = 3;
/// A decor model or skin finished loading, keyed by its URL's FNV-1a hash.
pub const DECOR_ASSET: Topic = 4;

/// `WorldTopic`.
pub type WorldTopic = Topic;

/// The kernel's log.
pub fn world_changes() -> &'static mut ChangeLog {
    &mut kernel().changes
}

/// The tasks the kernel spawns (sched/turn.rs's `world_tasks`), with room: the
/// edge parks each at most once, and an executor sizes its pool by it.
pub const TASKS: usize = 8;

static EDGE: Global<Edge<TASKS>> = Global::new();

/// The kernel's side of the turn (xpute_runtime::sched::edge): the doorbell
/// rises and falls it, and a task yields against it.
pub fn world_edge() -> &'static mut Edge<TASKS> {
    EDGE.get(Edge::new)
}

/// The falling edge. A task queued and not yet polled — a turn with no frame
/// polls none — asks for the next turn too.
pub fn world_fall() -> f64 {
    if xpute_runtime::sched::task::pending() {
        world_edge().ask_next();
    }
    world_edge().fall()
}

/// Asks for the frame that delivers changes made in the kernel — the store's
/// own bumps — the way bump_world does for a change made here.
pub fn request_world_tick() {
    world_edge().ask_next();
}

/// Asks for a turn once the host clock reaches `at`: a wait only time ends —
/// a debounce, a retry, a dismissal. The earliest asked wins, and a wait
/// still pending when its turn comes asks again.
pub fn wake_at(at: f64) {
    world_edge().wake_at(at);
}

/// Records a change to world state and asks for the frame that delivers it.
pub fn bump_world(topic: WorldTopic, key: u64) -> Revision {
    let revision = world_changes().bump(topic, key as i64);
    request_world_tick();
    revision
}
