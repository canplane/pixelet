// pxl-kernel/domain/layer/terrain/pool.rs

//! The ways' pool (pxl-engine `way::tree`): every brick's tree, a run of child
//! groups each, laid here as its fill ends (clipmap.rs `fill_stage`) and given
//! back with its slot. A cursor reads it, and the GPU's pool is a copy, each
//! run handed as its brick is (clipmap.rs `upload`, pxl-render gpu/ray/ground.rs
//! `hand`), the words as they lie, so what the runtime holds is not written
//! (`in_flight`). On the heap, a chunk at a time as the runs reach past it, a
//! child group's sixteen words each, never past the pool's most (pxl-engine
//! way/tree.rs `POOL_CHILD_GROUPS`).

use pxl_engine::clipmap::SLOTS;
use pxl_engine::overwrite::Columns;
use pxl_engine::way::tree::{self, Pool, BRICK_CHILD_GROUPS, CHILD_GROUP_WORDS};
use pxl_render::gpu::api;
use xpute_core::global::Global;
use xpute_kit::collection::list::List;

/// A chunk's child groups, 64 KiB of them: what the pool grows by.
const CHUNK_CHILD_GROUPS: u32 = 1024;
const CHUNK_WORDS: usize = (CHUNK_CHILD_GROUPS * CHILD_GROUP_WORDS) as usize;

/// The pool at the cap as the heap holds it, in the buddy's blocks: its
/// chunks, each a power of two, and the list of them
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
pub const POOL_BYTES: u64 = {
    let chunks = tree::POOL_CHILD_GROUPS.div_ceil(CHUNK_CHILD_GROUPS) as u64;
    chunks * (CHUNK_WORDS as u64 * 4).next_power_of_two()
        + (chunks * size_of::<Box<[u32]>>() as u64).next_power_of_two()
};
/// A run of child groups: the first and how many, none where it holds none.
pub type Run = [u32; 2];

/// The pieces a run lies in at its most, a chunk each.
pub const RUN_PIECES: usize = (BRICK_CHILD_GROUPS / CHUNK_CHILD_GROUPS) as usize + 2;

/// The chunks, child group after child group, each its eight child pointers then its eight
/// data words; a child group past them reads zero, nothing, as one never laid. Past `top`
/// none was taken, and the runs given back below it are `free`, lowest first, none
/// touching another: no more than the bricks held and one, each lying between two.
pub struct WayPool {
    chunks: Vec<Box<[u32]>>,
    top: u32,
    free: List<Run, { SLOTS as usize + 1 }>,
}

impl Default for WayPool {
    /// Child group 0 is none's (`way::tree::root_word`).
    fn default() -> WayPool {
        WayPool {
            chunks: Vec::new(),
            top: 1,
            free: List::new(),
        }
    }
}

static POOL: Global<WayPool> = Global::new();

/// Why a brick's child groups were not taken: they lie past the pool's cap,
/// which asking again never changes; or the heap refused a chunk, which a
/// page given back may (sys/io/room.rs `Refusal`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unlaid {
    Cap,
    Heap,
}

/// The pool, empty before any brick is filled.
pub fn way_pool() -> &'static mut WayPool {
    POOL.get(WayPool::default)
}

impl WayPool {
    /// A run of `n` child groups: the lowest free that holds them, else past
    /// every other, the chunks they reach grown first.
    pub fn take(&mut self, n: u32) -> Result<Run, Unlaid> {
        if let Some(k) = self.free.iter().position(|r| r[1] >= n) {
            let [first, cnt] = self.free[k];
            if cnt == n {
                self.free.remove(k);
            } else {
                self.free[k] = [first + n, cnt - n];
            }
            return Ok([first, n]);
        }
        self.grow_to(self.top + n - 1)?;
        let first = self.top;
        self.top += n;
        Ok([first, n])
    }

    /// A run taken given back, joined to those it touches.
    pub fn give(&mut self, [first, cnt]: Run) {
        if cnt == 0 {
            return;
        }
        let k = self.free.iter().position(|r| r[0] > first);
        let k = k.unwrap_or(self.free.len());
        self.free.push([first, cnt]);
        self.free[k..].rotate_right(1);
        if k + 1 < self.free.len() && self.free[k][0] + self.free[k][1] == self.free[k + 1][0] {
            self.free[k][1] += self.free[k + 1][1];
            self.free.remove(k + 1);
        }
        if k > 0 && self.free[k - 1][0] + self.free[k - 1][1] == self.free[k][0] {
            self.free[k - 1][1] += self.free[k][1];
            self.free.remove(k);
        }
    }

    /// The child groups the pool spans, what the GPU's must hold.
    pub fn top(&self) -> u32 {
        self.top
    }

    /// `run` as it lies, a piece a chunk it reaches: each its first child group
    /// and its words.
    pub fn pieces(&self, [first, cnt]: Run) -> List<(u32, &[u32]), RUN_PIECES> {
        let mut pieces = List::new();
        let (mut at, end) = (first, first + cnt);
        while at < end {
            let past = (at / CHUNK_CHILD_GROUPS + 1) * CHUNK_CHILD_GROUPS;
            let until = past.min(end);
            let chunk = &self.chunks[(at / CHUNK_CHILD_GROUPS) as usize];
            let word = |g: u32| ((g % CHUNK_CHILD_GROUPS) * CHILD_GROUP_WORDS) as usize;
            let last = word(until - 1) + CHILD_GROUP_WORDS as usize;
            pieces.push((at, &chunk[word(at)..last]));
            at = until;
        }
        pieces
    }

    /// Whether the runtime holds any of `run` (`in_flight`): nothing lays there
    /// until it gives it back.
    pub fn in_flight(&self, run: Run) -> bool {
        self.pieces(run).iter().any(|p| api::in_flight(p.1))
    }

    /// The chunks grown to hold child group `child_group`; refused where it lies past
    /// the cap or the heap refuses, the chunks grown so far kept.
    fn grow_to(&mut self, child_group: u32) -> Result<(), Unlaid> {
        if child_group >= tree::POOL_CHILD_GROUPS {
            return Err(Unlaid::Cap);
        }
        let need = (child_group / CHUNK_CHILD_GROUPS + 1) as usize;
        if need > self.chunks.len()
            && self
                .chunks
                .try_reserve_exact(need - self.chunks.len())
                .is_err()
        {
            return Err(Unlaid::Heap);
        }
        while self.chunks.len() < need {
            let mut chunk = Vec::new();
            if chunk.try_reserve_exact(CHUNK_WORDS).is_err() {
                return Err(Unlaid::Heap);
            }
            chunk.resize(CHUNK_WORDS, 0);
            self.chunks.push(chunk.into_boxed_slice());
        }
        Ok(())
    }

    /// Where word `k` of slot `slot`'s child group lies, the child pointer's from 0 and
    /// the data's from 8: its chunk and its index in it.
    fn at(slot: u32, k: u32) -> (usize, usize) {
        let child_group = slot / 8;
        (
            (child_group / CHUNK_CHILD_GROUPS) as usize,
            ((child_group % CHUNK_CHILD_GROUPS) * CHILD_GROUP_WORDS + k + slot % 8) as usize,
        )
    }

    fn word(&self, slot: u32, k: u32) -> u32 {
        let (c, i) = WayPool::at(slot, k);
        self.chunks.get(c).map_or(0, |chunk| chunk[i])
    }

    fn set(&mut self, slot: u32, k: u32, v: u32) {
        let (c, i) = WayPool::at(slot, k);
        if let Some(chunk) = self.chunks.get_mut(c) {
            chunk[i] = v;
        }
    }
}

impl Columns for WayPool {
    fn first_child(&self, slot: u32) -> u32 {
        self.word(slot, 0)
    }
    fn data(&self, slot: u32) -> u32 {
        self.word(slot, 8)
    }
}

impl Pool for WayPool {
    fn set_first_child(&mut self, slot: u32, v: u32) {
        self.set(slot, 0, v);
    }
    fn set_data(&mut self, slot: u32, v: u32) {
        self.set(slot, 8, v);
    }
}

#[cfg(test)]
pub(crate) fn set_way_pool(m: WayPool) {
    POOL.set(m);
}

#[cfg(test)]
impl Clone for WayPool {
    fn clone(&self) -> WayPool {
        let mut free = List::new();
        self.free.iter().for_each(|&r| free.push(r));
        WayPool {
            chunks: self.chunks.clone(),
            top: self.top,
            free,
        }
    }
}

#[cfg(test)]
#[path = "pool.test.rs"]
mod test;
