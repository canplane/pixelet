// pxl-kernel/chunk/build.rs

//! A chunk's build: makes its tiers from its sources (source.rs). Every
//! decision about a chunk is the table's (table.rs); this acts on what it asks.

mod bake;
mod shape;

pub use bake::BAKE_BYTES;
pub use shape::draws_as_held;

use bake::BakeJob;

use pxl_render::mesh::chunk::{BakeContext, Heights};
use pxl_spec::block::sample::Blocks;

use crate::Build;

use crate::chunk::mesh_slot::OCCUPANCY;
use crate::chunk::table::builds;
use crate::chunk::Chunk as PyramidChunk;
use crate::sched::edge::{request_world_tick, world_edge};
use crate::with_parts;
use xpute_guest::sched::edge::{notified, yield_if_spent};
use xpute_kit::status::bug::OrBug;
use xpute_kit::status::errno::Errno;

/// `.fixed.bake`, handed in at boot.
pub static BAKE_RANGE: crate::mem::fixed::Held = crate::mem::fixed::Held::new();

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BuildEvent {
    /// All slots for a tier are asked for at once, before any is written. If
    /// refused, the bake is thrown away and the need recorded (`Slot::need`).
    Reserve {
        id: u32,
        generation: u32,
        boxes: u32,
    },
    /// One slot's worth of boxes (less for the last) at `at` in `.fixed.bake`.
    Part {
        id: u32,
        generation: u32,
        at: u32,
        first: u32,
        box_cnt: u32,
        heights: Heights,
    },
    Tier {
        id: u32,
        generation: u32,
        box_cnt: u32,
        heights: Heights,
    },
}

#[derive(Clone, Copy)]
struct Chunk {
    at: PyramidChunk,
    generation: u32,
    /// Its tier is asked for.
    want: bool,
    /// Heap release count at the last refusal: until something is freed the same
    /// request would fail, so the chunk is skipped.
    refused: Option<u32>,
}

#[derive(PartialEq, Eq)]
enum Progress {
    Busy,
    Done,
    /// Waiting on another chunk's slot, a refused heap block, or a tile.
    Blocked,
}

/// Per-mount state, sized once for every mount the table can give.
struct ByMount<T>(Vec<Option<T>>);

impl<T> ByMount<T> {
    fn new() -> ByMount<T> {
        ByMount((0..OCCUPANCY).map(|_| None).collect())
    }

    fn get(&self, id: &u32) -> Option<&T> {
        self.0.get(*id as usize)?.as_ref()
    }

    fn get_mut(&mut self, id: &u32) -> Option<&mut T> {
        self.0.get_mut(*id as usize)?.as_mut()
    }

    fn insert(&mut self, id: u32, t: T) {
        self.0[id as usize] = Some(t);
    }

    fn remove(&mut self, id: &u32) {
        self.0[*id as usize] = None;
    }
}

impl<T> core::ops::Index<&u32> for ByMount<T> {
    type Output = T;

    fn index(&self, id: &u32) -> &T {
        self.get(id).or_bug(Errno::ENOTRECOVERABLE)
    }
}

#[derive(Clone, Copy, Default)]
pub struct BuildStat {
    pub bakes: u32,
    pub starved: u32,
}

pub struct Builder {
    chunks: ByMount<Chunk>,
    bake: BakeContext,
    /// The one open bake, and what it reads: its chunk's blocks, composed
    /// when it begins and held until it ends.
    bake_job: Option<BakeJob>,
    blocks: Blocks,
    pub stat: BuildStat,
}

impl Default for Builder {
    fn default() -> Builder {
        Builder {
            chunks: ByMount::new(),
            bake: BakeContext::default(),
            bake_job: None,
            blocks: Blocks::default(),
            stat: BuildStat::default(),
        }
    }
}

impl Builder {
    pub fn generation_of(&self, id: u32) -> Option<u32> {
        self.chunks.get(&id).map(|c| c.generation)
    }

    pub fn holder(&self) -> Option<u32> {
        self.bake_job.as_ref().map(|j| j.id)
    }

    pub fn add(&mut self, id: u32, at: PyramidChunk) {
        self.chunks.insert(
            id,
            Chunk {
                at,
                generation: 0,
                want: false,
                refused: None,
            },
        );
    }

    pub fn remove(&mut self, id: u32) {
        self.abort_work_of(id);
        self.chunks.remove(&id);
    }

    pub fn source(&mut self, id: u32) {
        self.abort_work_of(id);
        if let Some(c) = self.chunks.get_mut(&id) {
            c.generation = c.generation.wrapping_add(1);
        }
    }

    pub fn baking(&self) -> Option<(u32, u32)> {
        self.bake_job.as_ref().map(|j| (j.id, j.generation))
    }

    /// A bake of a tier no longer wanted gives its slot back now.
    pub fn want(&mut self, id: u32, want: bool) {
        let Some(c) = self.chunks.get_mut(&id) else { return };
        c.want = want;
        if !want {
            self.abort_work_of(id);
        }
    }

    fn abort_work_of(&mut self, id: u32) {
        if matches!(&self.bake_job, Some(j) if j.id == id) {
            self.abort_bake();
        }
    }

    /// Heap refused a block: give up every slot held, since each is another
    /// chunk's wait, and skip the chunk until a release.
    fn refuse(&mut self, id: u32) {
        self.stat.starved += 1;
        self.abort_work_of(id);
        let releases = crate::io::host::heap_stat().releases;
        if let Some(c) = self.chunks.get_mut(&id) {
            c.refused = Some(releases);
        }
    }

    fn unwant(&mut self, id: u32) {
        if let Some(c) = self.chunks.get_mut(&id) {
            c.want = false;
        }
    }

    /// Advances chunks under the budget, the bake's holder first. At least
    /// one step is taken. A heap refusal ends the tick: the heap will not
    /// change within it.
    pub fn tick(&mut self, build: &mut Build, order: &[u32], budget_ms: f64, now: &impl Fn() -> f64, emit: &mut impl FnMut(BuildEvent) -> bool) -> bool {
        let t0 = now();
        let starved = self.stat.starved;
        let mut stepped = false;
        let mut worked = false;
        let first = self.holder();
        for id in first.into_iter().chain(order.iter().copied().filter(|&id| Some(id) != first)) {
            loop {
                if stepped && now() - t0 >= budget_ms {
                    return worked;
                }
                let progress = self.advance(build, id, emit);
                if self.stat.starved != starved {
                    return worked;
                }
                if progress != Progress::Blocked {
                    stepped = true;
                }
                if progress != Progress::Busy {
                    break;
                }
                worked = true;
            }
        }
        worked
    }

    fn advance(&mut self, build: &mut Build, id: u32, emit: &mut impl FnMut(BuildEvent) -> bool) -> Progress {
        let Some(c) = self.chunks.get(&id) else { return Progress::Done };
        if !c.want {
            return Progress::Done;
        }
        if c.refused.is_some_and(|r| r == crate::io::host::heap_stat().releases) {
            return Progress::Blocked;
        }
        self.advance_bake(build, id, emit)
    }
}

/// Advances the builds on what the quota leaves after the tick, at least one
/// step a turn, and waits for the next tick once a step does nothing.
pub async fn build_task() {
    loop {
        notified(builds).await;
        loop {
            yield_if_spent(world_edge).await;
            let budget = world_edge().remaining();
            let worked = with_parts(|build, sched, _| sched.build_step(build, budget)).unwrap_or(false);
            if !worked {
                break;
            }
            request_world_tick();
        }
    }
}
