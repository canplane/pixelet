// pxl-core/domain/layer/terrain/pool.rs

//! The ways' pool (pxl-engine `clipmap::WAYS_POOL_WORDS`): one bounded allocator of words on
//! the heap, holding what the GPU reads and nothing else. Each brick's kept spans are a run here
//! (clipmap.rs `lay`), given back with the brick's slot, and the GPU's copy follows it: a brick's
//! run is handed over as its brick is (clipmap.rs `upload`, pxl-render ray/ground.rs `hand`), the
//! words read in place, so `take` passes over a free run the runtime still holds. A brick with
//! nothing to keep takes nothing.
//!
//! The pool grows a chunk at a time as runs reach past its end. A run may cross chunks: readers
//! index it by word, and the GPU is handed its words piece by piece (`pieces`). Only the pool's
//! cap or the heap refuses. Word 0 is never handed out: a brick's `ROOT_AT` of 0 names no run.

use pxl_engine::clipmap::{Pool, RUN_WORDS_MOST, SLOTS, WAYS_POOL_WORDS};
use pxl_render::gpu::api;
use xpute_core::global::Global;
use xpute_kit::collection::list::List;

use crate::sys::io::room;

/// The words in a chunk, 64 KiB, which is the step the pool grows by.
const CHUNK_WORDS: u32 = 1 << 14;
const CHUNK_CNT: u32 = WAYS_POOL_WORDS / CHUNK_WORDS;

/// The heap the pool takes at its cap, counted in the buddy allocator's blocks: each chunk
/// rounded up to a power of two, and the list of chunks likewise (door/layout.rs
/// `HEAP_FLOWS_BYTES`).
pub const POOL_BYTES: u64 = (CHUNK_CNT as u64) * (CHUNK_WORDS as u64 * 4).next_power_of_two()
    + (CHUNK_CNT as u64 * size_of::<Box<[u32]>>() as u64).next_power_of_two();

/// A run of words: the first and how many. A count of zero means the run holds none.
pub(in crate::domain::layer) type Run = [u32; 2];

/// The most pieces a run lies in, one a chunk it reaches.
pub(in crate::domain::layer) const RUN_PIECES: usize = (RUN_WORDS_MOST / CHUNK_WORDS) as usize + 2;

/// The runs given back below `top` are in `free`, lowest first, and no two of them touch, so
/// each lies between two held runs and there is at most one more of them than there are
/// bricks.
pub(in crate::domain::layer) struct WayPool {
    chunks: Vec<Box<[u32]>>,
    top: u32,
    free: List<Run, { SLOTS as usize + 2 }>,
}

impl Default for WayPool {
    /// Word 0 is never handed out (`clipmap::ROOT_AT`).
    fn default() -> WayPool {
        WayPool {
            chunks: Vec::new(),
            top: 1,
            free: List::new(),
        }
    }
}

static POOL: Global<WayPool> = Global::new();

/// The ways' pool, holding the spans of every held brick. The world's query reads it as the
/// GPU's cast reads its copy (pxl-engine `clipmap::way_at`).
pub(in crate::domain::layer) fn way_pool() -> &'static mut WayPool {
    POOL.get(WayPool::default)
}

/// Why a run was not taken. `Cap`: the pool is full to its cap, and a run given back may change
/// that (sys/io/room.rs `Refusal::ways`). `Heap`: the heap refused a chunk, and a page given back
/// may change that (`Refusal::now`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::domain::layer) enum PoolRefusal {
    Cap,
    Heap,
}

impl WayPool {
    /// Takes a run of `n` words from the lowest free run that holds it and that the runtime does
    /// not still hold (`in_flight`), or else past every other run, growing the chunks it reaches
    /// first. `n` is at least 1.
    pub fn take(&mut self, n: u32) -> Result<Run, PoolRefusal> {
        if let Some(k) = (0..self.free.len())
            .find(|&k| self.free[k][1] >= n && !self.in_flight([self.free[k][0], n]))
        {
            let [first, cnt] = self.free[k];
            if cnt == n {
                self.free.remove(k);
            } else {
                self.free[k] = [first + n, cnt - n];
            }
            return Ok([first, n]);
        }
        if self.top + n > WAYS_POOL_WORDS {
            return Err(PoolRefusal::Cap);
        }
        self.grow_to(self.top + n - 1)?;
        let first = self.top;
        self.top += n;
        Ok([first, n])
    }

    /// Gives back a run taken, joined with the free runs it touches, which counts as room given
    /// back for a refusal (`Refusal::ways`).
    pub fn give(&mut self, run: Run) {
        if run[1] == 0 {
            return;
        }
        self.insert(run);
        room::way_given_back();
    }

    /// Joins `[first, cnt]` with the free runs it touches.
    fn insert(&mut self, [first, cnt]: Run) {
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

    /// The words the pool spans, which the GPU's copy must hold.
    pub fn top(&self) -> u32 {
        self.top
    }

    /// Whether a run of `n` words could be taken now, the heap's say apart.
    pub fn fits(&self, n: u32) -> bool {
        self.free.iter().any(|r| r[1] >= n)
            || n.checked_add(self.top)
                .is_some_and(|sum| sum <= WAYS_POOL_WORDS)
    }

    /// `run` in place, one piece for each chunk it reaches, each piece with its first word in
    /// the pool and its words.
    pub fn pieces(&self, [first, cnt]: Run) -> List<(u32, &[u32]), RUN_PIECES> {
        let mut pieces = List::new();
        let (mut at, end) = (first, first + cnt);
        while at < end {
            let until = ((at / CHUNK_WORDS + 1) * CHUNK_WORDS).min(end);
            let chunk = &self.chunks[(at / CHUNK_WORDS) as usize];
            let from = (at % CHUNK_WORDS) as usize;
            pieces.push((at, &chunk[from..from + (until - at) as usize]));
            at = until;
        }
        pieces
    }

    /// Writes word `k` of the pool, across chunks.
    pub fn set(&mut self, k: u32, v: u32) {
        if let Some(chunk) = self.chunks.get_mut((k / CHUNK_WORDS) as usize) {
            chunk[(k % CHUNK_WORDS) as usize] = v;
        }
    }

    /// Whether the runtime holds any of `run` (pxl-render `api::in_flight`).
    fn in_flight(&self, run: Run) -> bool {
        #[cfg(test)]
        if TEST_HELD.get(|| [0; 2]) == &run {
            return true;
        }
        self.pieces(run).iter().any(|p| api::in_flight(p.1))
    }

    /// Grows the chunks to hold word `word`. It is refused where the heap refuses, and the
    /// chunks grown so far are kept.
    fn grow_to(&mut self, word: u32) -> Result<(), PoolRefusal> {
        let need = (word / CHUNK_WORDS + 1) as usize;
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
            if chunk.try_reserve_exact(CHUNK_WORDS as usize).is_err() {
                return Err(PoolRefusal::Heap);
            }
            chunk.resize(CHUNK_WORDS as usize, 0);
            self.chunks.push(chunk.into_boxed_slice());
        }
        Ok(())
    }
}

/// The pool as pxl-engine's query reads it (`clipmap::way_at`).
impl Pool for WayPool {
    fn word(&self, k: u32) -> u32 {
        self.chunks
            .get((k / CHUNK_WORDS) as usize)
            .map_or(0, |chunk| chunk[(k % CHUNK_WORDS) as usize])
    }
}

/// A run a test holds as the runtime would (`in_flight`).
#[cfg(test)]
pub(in crate::domain::layer) static TEST_HELD: Global<Run> = Global::new();

#[cfg(test)]
pub(in crate::domain::layer) fn set_way_pool(m: WayPool) {
    POOL.set(m);
}

/// Leaves no room below the cap, as if every word were taken.
#[cfg(test)]
pub(in crate::domain::layer) fn exhaust(m: &mut WayPool) {
    m.top = WAYS_POOL_WORDS - 1;
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
