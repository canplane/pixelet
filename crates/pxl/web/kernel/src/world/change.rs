// pxl-kernel/world/change.rs

//! Revisions of world-shared state. One tick per topic rather than per key,
//! so a key forgotten and changed again never repeats a revision a reader
//! holds (Bevy's change ticks). A wrap of u32 is not handled. A bump also
//! asks for a frame, since the canvas renders on demand.

use std::collections::HashMap;

use fnv::FnvBuildHasher;

use xpute_runtime::sched::edge::Edge;

use crate::kernel::kernel;
use xpute_runtime::global::Global;

pub const OBJECT_COLUMN: WorldTopic = 0;
/// Moved by a write to any column the chunk draws, so a chunk reads one
/// revision rather than summing its columns'.
pub const TIERS_CHUNK: WorldTopic = 1;
pub const WORLD_TOPICS: WorldTopic = TIERS_CHUNK + 1;

pub type WorldTopic = u32;
pub type Revision = u32;

#[derive(Default)]
pub struct Revisions {
    ticks: [Revision; WORLD_TOPICS as usize],
    keys: [HashMap<i64, Revision, FnvBuildHasher>; WORLD_TOPICS as usize],
}

impl Revisions {
    pub fn bump(&mut self, topic: WorldTopic, key: i64) -> Revision {
        let tick = self.ticks[topic as usize].wrapping_add(1);
        self.ticks[topic as usize] = tick;
        self.keys[topic as usize].insert(key, tick);
        tick
    }

    pub fn revision_of(&self, topic: WorldTopic, key: i64) -> Revision {
        self.keys[topic as usize].get(&key).copied().unwrap_or(0)
    }

    pub fn tick(&self, topic: WorldTopic) -> Revision {
        self.ticks[topic as usize]
    }
}

pub fn world_changes() -> &'static mut Revisions {
    &mut kernel().changes
}

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

pub fn bump_world(topic: WorldTopic, key: u64) -> Revision {
    let revision = world_changes().bump(topic, key as i64);
    request_world_tick();
    revision
}
