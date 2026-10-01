// pxl-kernel/world/change.rs

//! The world's revisions: which units of world-shared state changed, read by
//! whoever cares (issue/20260913-the-read-direction-a-change-log.md). A
//! revision is its topic's tick at the key's last change, one counter per
//! topic rather than per key, so a key forgotten and changed again gets a
//! revision it never had and a reader holding an old one cannot mistake the
//! new state for the one it saw — Bevy's change ticks, per topic. The tick is
//! u32 and a wrap is not handled: four billion changes to one topic.
//!
//! A source calls `bump_world`; a reader compares a key's revision with the
//! one it remembers, or a topic's tick. A change carries no value: the value
//! is the owner's, read by key where it lives. The canvas renders on demand,
//! so a bump also asks for that frame; the request goes back to the host on
//! the turn's falling edge.

use std::collections::HashMap;

use fnv::FnvBuildHasher;

use xpute_runtime::sched::edge::Edge;

use crate::global::Global;
use crate::kernel::kernel;

/// What a column stores (object/store.rs), keyed by column index.
pub const OBJECT_COLUMN: WorldTopic = 0;
/// What a chunk above z16 draws of the objects in its tiers, keyed by the
/// chunk (world/chunk/source.rs `chunk_key`): moved by a write to any column
/// it draws, so a chunk reads one revision rather than summing its columns'.
pub const TIERS_CHUNK: WorldTopic = 1;
/// How many topics there are.
pub const WORLD_TOPICS: WorldTopic = TIERS_CHUNK + 1;

pub type WorldTopic = u32;
pub type Revision = u32;

/// Each topic's tick, and each key's revision in it.
#[derive(Default)]
pub struct Revisions {
    ticks: [Revision; WORLD_TOPICS as usize],
    keys: [HashMap<i64, Revision, FnvBuildHasher>; WORLD_TOPICS as usize],
}

impl Revisions {
    /// Records that `key` in `topic` changed, and returns its new revision.
    pub fn bump(&mut self, topic: WorldTopic, key: i64) -> Revision {
        let tick = self.ticks[topic as usize].wrapping_add(1);
        self.ticks[topic as usize] = tick;
        self.keys[topic as usize].insert(key, tick);
        tick
    }

    /// The key's revision, or 0 where it has never changed.
    pub fn revision_of(&self, topic: WorldTopic, key: i64) -> Revision {
        self.keys[topic as usize].get(&key).copied().unwrap_or(0)
    }

    /// The topic's tick: moved means something in the topic changed.
    pub fn tick(&self, topic: WorldTopic) -> Revision {
        self.ticks[topic as usize]
    }
}

/// The kernel's revisions.
pub fn world_changes() -> &'static mut Revisions {
    &mut kernel().changes
}

/// The tasks the kernel spawns (sched/turn.rs's `world_tasks`), with room: the
/// edge parks each at most once, and an executor sizes its pool by it.
pub const TASKS: usize = 6;

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
