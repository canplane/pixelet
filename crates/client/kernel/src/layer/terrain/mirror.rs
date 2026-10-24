// pxl-kernel/layer/terrain/mirror.rs

//! The GPU's ways' pool as the CPU mirrors it (pxl-spec-core `ways::tree`):
//! each landing square's blocks laid at the ids the GPU gave them
//! (window.rs `landed`), which a cursor reads as the cast reads the GPU's.
//! On the heap, a chunk at a time as the ids laid reach past it: the GPU's
//! stack hands ids from the lowest up (pxl-shader `TAKEN`), so the mirror
//! holds the blocks up to the most ever taken at once, a block's sixteen
//! words each, never past the pool's cap (param `WAYS_POOL_BYTES`).

use pxl_spec_core::overwrite::Lanes;
use pxl_spec_core::ways::tree::{self, Pool, BLOCK_WORDS, RECORD_WORDS, SQUARE_BLOCKS};
use xpute_guest::global::Global;

/// A chunk's blocks, 64 KiB of them: what the mirror grows by.
const CHUNK_BLOCKS: u32 = 1024;
const CHUNK_WORDS: usize = (CHUNK_BLOCKS * BLOCK_WORDS) as usize;

/// The blocks the cap holds.
const MOST_BLOCKS: u32 = crate::param::WAYS_POOL_BYTES / (4 * BLOCK_WORDS);

/// The mirror at the cap as the heap holds it, in the buddy's blocks: its
/// chunks, each a power of two, and the list of them
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
pub const MIRROR_BYTES: u64 = {
    let chunks = MOST_BLOCKS.div_ceil(CHUNK_BLOCKS) as u64;
    chunks * (CHUNK_WORDS as u64 * 4).next_power_of_two() + (chunks * size_of::<Box<[u32]>>() as u64).next_power_of_two()
};
/// The chunks, block after block, each its eight heap words then its eight
/// data words; a block past them reads zero, nothing, as one never laid.
#[derive(Default)]
pub struct Mirror {
    chunks: Vec<Box<[u32]>>,
}

static MIRROR: Global<Mirror> = Global::new();

/// The mirror, empty before any square lands.
pub fn mirror() -> &'static mut Mirror {
    MIRROR.get(Mirror::default)
}

impl Mirror {
    /// A square's records (pxl-spec-core `ways::tree::lay`) laid at their
    /// blocks' ids, the chunks they reach grown first: false, nothing laid,
    /// where one lies past the cap or the heap refuses a chunk, which the
    /// square's landing takes as its tree refused.
    pub fn lay(&mut self, records: &[u32]) -> bool {
        let n = records.first().map_or(0, |&n| n.min(SQUARE_BLOCKS));
        let id = |r: u32| records.get((1 + r * RECORD_WORDS) as usize).copied().unwrap_or(0);
        let most = (0..n).map(id).max().unwrap_or(0);
        if !self.reach(most) {
            return false;
        }
        tree::lay(self, records);
        true
    }

    /// The chunks grown to hold block `block`; false where it lies past the
    /// cap or the heap refuses, the chunks grown so far kept.
    fn reach(&mut self, block: u32) -> bool {
        if block >= MOST_BLOCKS {
            return false;
        }
        let need = (block / CHUNK_BLOCKS + 1) as usize;
        if need > self.chunks.len() && self.chunks.try_reserve_exact(need - self.chunks.len()).is_err() {
            return false;
        }
        while self.chunks.len() < need {
            let mut chunk = Vec::new();
            if chunk.try_reserve_exact(CHUNK_WORDS).is_err() {
                return false;
            }
            chunk.resize(CHUNK_WORDS, 0);
            self.chunks.push(chunk.into_boxed_slice());
        }
        true
    }

    /// Every block let go: the device that held the pool went, every
    /// square with it, and nothing reads a tree until one lands again.
    pub fn clear(&mut self) {
        self.chunks = Vec::new();
    }

    /// Where word `k` of slot `slot`'s block lies, the heap's from 0 and
    /// the data's from 8: its chunk and its index in it.
    fn at(slot: u32, k: u32) -> (usize, usize) {
        let block = slot / 8;
        ((block / CHUNK_BLOCKS) as usize, ((block % CHUNK_BLOCKS) * BLOCK_WORDS + k + slot % 8) as usize)
    }

    fn word(&self, slot: u32, k: u32) -> u32 {
        let (c, i) = Mirror::at(slot, k);
        self.chunks.get(c).map_or(0, |chunk| chunk[i])
    }

    fn set(&mut self, slot: u32, k: u32, v: u32) {
        let (c, i) = Mirror::at(slot, k);
        if let Some(chunk) = self.chunks.get_mut(c) {
            chunk[i] = v;
        }
    }
}

impl Lanes for Mirror {
    fn heap(&self, slot: u32) -> u32 {
        self.word(slot, 0)
    }
    fn data(&self, slot: u32) -> u32 {
        self.word(slot, 8)
    }
}

impl Pool for Mirror {
    fn set_heap(&mut self, slot: u32, v: u32) {
        self.set(slot, 0, v);
    }
    fn set_data(&mut self, slot: u32, v: u32) {
        self.set(slot, 8, v);
    }
}

#[cfg(test)]
#[path = "mirror.test.rs"]
mod test;
