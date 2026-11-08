// pxl-core/domain/layer/terrain/pool.rs

//! The ways' pool (pxl-engine `way::tree`). Each brick's tree of ways is a
//! run of child groups, laid here when the brick's fill ends (clipmap.rs
//! `fill_stage`) and given back with the brick's slot. A cursor reads the
//! pool here, and the GPU's pool is a copy: each run is handed over as its
//! brick is (clipmap.rs `upload`, pxl-render gpu/ray/ground.rs `hand`), with
//! the words in place, so nothing may write a run the runtime holds
//! (`in_flight`).
//!
//! The pool lives on the heap and grows a chunk at a time as the runs reach
//! past its end. A child group is sixteen words. The pool never grows past
//! its cap (pxl-engine way/tree.rs `POOL_CHILD_GROUPS`).

use pxl_engine::clipmap::SLOTS;
use pxl_engine::object::Columns;
use pxl_engine::way::tree::{self, Pool, BRICK_CHILD_GROUPS, CHILD_GROUP_WORDS};
use pxl_render::gpu::api;
use xpute_core::global::Global;
use xpute_kit::collection::list::List;

/// The child groups in a chunk, 64 KiB of words, which is the step the pool
/// grows by.
const CHUNK_CHILD_GROUPS: u32 = 1024;
const CHUNK_WORDS: usize = (CHUNK_CHILD_GROUPS * CHILD_GROUP_WORDS) as usize;

/// The heap the pool takes at its cap, counted in the buddy allocator's
/// blocks: each chunk rounded up to a power of two, and the list of chunks
/// likewise (door/layout.rs `HEAP_FLOWS_BYTES`).
pub const POOL_BYTES: u64 = {
    let chunks = tree::POOL_CHILD_GROUPS.div_ceil(CHUNK_CHILD_GROUPS) as u64;
    chunks * (CHUNK_WORDS as u64 * 4).next_power_of_two()
        + (chunks * size_of::<Box<[u32]>>() as u64).next_power_of_two()
};
/// A run of child groups: the first one and how many. A count of zero means
/// the run holds none.
pub type Run = [u32; 2];

/// The most pieces a run lies in, one piece for each chunk it reaches.
pub const RUN_PIECES: usize = (BRICK_CHILD_GROUPS / CHUNK_CHILD_GROUPS) as usize + 2;

/// The chunks hold child group after child group, each with its eight child
/// pointers and then its eight data words. A child group past the chunks
/// reads as zero, which means nothing, like one never laid. No child group
/// at or past `top` was ever taken. The runs given back below `top` are in
/// `free`, lowest first, and no two of them touch, so each lies between two
/// held runs and there is at most one more of them than there are bricks.
pub struct WayPool {
    chunks: Vec<Box<[u32]>>,
    top: u32,
    free: List<Run, { SLOTS as usize + 1 }>,
}

impl Default for WayPool {
    /// Child group 0 is never handed out, since a root word of 0 stands for
    /// a brick with no tree (`way::tree::root_word`).
    fn default() -> WayPool {
        WayPool {
            chunks: Vec::new(),
            top: 1,
            free: List::new(),
        }
    }
}

static POOL: Global<WayPool> = Global::new();

/// Why a brick's child groups were not taken. `Cap`: they lie past the
/// pool's cap, and asking again never changes that. `Heap`: the heap refused
/// a chunk, and a page given back may change that (sys/io/room.rs
/// `Refusal`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolRefusal {
    Cap,
    Heap,
}

/// The pool, empty before any brick is filled.
pub fn way_pool() -> &'static mut WayPool {
    POOL.get(WayPool::default)
}

impl WayPool {
    /// Takes a run of `n` child groups from the lowest free run that holds
    /// them, or else past every other run, growing the chunks it reaches
    /// first.
    pub fn take(&mut self, n: u32) -> Result<Run, PoolRefusal> {
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

    /// Gives back a run taken, joined with the free runs it touches.
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

    /// The child groups the pool spans, which the GPU's copy must hold.
    pub fn top(&self) -> u32 {
        self.top
    }

    /// `run` in place, one piece for each chunk it reaches, each piece with
    /// its first child group and its words.
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

    /// Whether the runtime holds any of `run` (pxl-render `api::in_flight`).
    /// Nothing may be laid there until the runtime gives it back.
    pub fn in_flight(&self, run: Run) -> bool {
        self.pieces(run).iter().any(|p| api::in_flight(p.1))
    }

    /// Grows the chunks to hold child group `child_group`. It is refused
    /// where that lies past the cap or the heap refuses, and the chunks grown
    /// so far are kept.
    fn grow_to(&mut self, child_group: u32) -> Result<(), PoolRefusal> {
        if child_group >= tree::POOL_CHILD_GROUPS {
            return Err(PoolRefusal::Cap);
        }
        let need = (child_group / CHUNK_CHILD_GROUPS + 1) as usize;
        if need > self.chunks.len()
            && self
                .chunks
                .try_reserve_exact(need - self.chunks.len())
                .is_err()
        {
            return Err(PoolRefusal::Heap);
        }
        while self.chunks.len() < need {
            let mut chunk = Vec::new();
            if chunk.try_reserve_exact(CHUNK_WORDS).is_err() {
                return Err(PoolRefusal::Heap);
            }
            chunk.resize(CHUNK_WORDS, 0);
            self.chunks.push(chunk.into_boxed_slice());
        }
        Ok(())
    }

    /// Where word `k` of slot `slot`'s child group lies, as its chunk and its
    /// index in the chunk. `k` is 0 for the slot's child pointer and 8 for
    /// its data word.
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
