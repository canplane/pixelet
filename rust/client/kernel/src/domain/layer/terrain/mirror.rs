// pxl-kernel/domain/layer/terrain/mirror.rs

//! The GPU's ways' pool as the CPU mirrors it (pxl-spec-core `way::tree`):
//! each landing brick's families laid at the ids the GPU gave them
//! (window.rs `on_landed`), which a cursor reads as the cast reads the GPU's.
//! On the heap, a chunk at a time as the ids laid reach past it: the GPU's
//! stack hands ids from the lowest up (pxl-shader `TAKEN`), so the mirror
//! holds the families up to the most ever taken at once, a family's sixteen
//! words each, never past the pool's most (pxl-spec-core way/tree.rs `POOL_FAMILIES`).

use pxl_spec_core::overwrite::Lanes;
use pxl_spec_core::way::tree::{self, Pool, BRICK_FAMILIES, FAMILY_WORDS, RECORD_WORDS};
use xpute_guest::global::Global;

/// A chunk's families, 64 KiB of them: what the mirror grows by.
const CHUNK_FAMILIES: u32 = 1024;
const CHUNK_WORDS: usize = (CHUNK_FAMILIES * FAMILY_WORDS) as usize;

/// The mirror at the cap as the heap holds it, in the buddy's blocks: its
/// chunks, each a power of two, and the list of them
/// (door/layout.rs `HEAP_FLOWS_BYTES`).
pub const MIRROR_BYTES: u64 = {
    let chunks = tree::POOL_FAMILIES.div_ceil(CHUNK_FAMILIES) as u64;
    chunks * (CHUNK_WORDS as u64 * 4).next_power_of_two()
        + (chunks * size_of::<Box<[u32]>>() as u64).next_power_of_two()
};
/// The chunks, family after family, each its eight heap words then its eight
/// data words; a family past them reads zero, nothing, as one never laid.
#[derive(Default)]
pub struct Mirror {
    chunks: Vec<Box<[u32]>>,
}

static MIRROR: Global<Mirror> = Global::new();

/// Why a brick's families were not laid: one lies past the pool's cap,
/// which asking again never changes; or the heap refused a chunk, which a
/// page given back may (sys/io/room.rs `Refused`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unlaid {
    Cap,
    Heap,
}

/// The mirror, empty before any brick lands.
pub fn mirror() -> &'static mut Mirror {
    MIRROR.get(Mirror::default)
}

impl Mirror {
    /// A brick's records (pxl-spec-core `way::tree::lay`) laid at their
    /// families' ids, the chunks they reach grown first; nothing laid where
    /// one lies past the cap or the heap refuses a chunk, which the brick's
    /// landing takes as its tree refused or waiting on the heap
    /// (domain/layer/terrain/window.rs `on_landed`). A brick with no ways has
    /// none, and reaches no chunk.
    pub fn lay(&mut self, records: &[u32]) -> Result<(), Unlaid> {
        let n = records.first().map_or(0, |&n| n.min(BRICK_FAMILIES));
        if n == 0 {
            return Ok(());
        }
        let id = |r: u32| {
            records
                .get((1 + r * RECORD_WORDS) as usize)
                .copied()
                .unwrap_or(0)
        };
        let most = (0..n).map(id).max().unwrap_or(0);
        self.reach(most)?;
        tree::lay(self, records);
        Ok(())
    }

    /// The chunks grown to hold family `family`; refused where it lies past
    /// the cap or the heap refuses, the chunks grown so far kept.
    fn reach(&mut self, family: u32) -> Result<(), Unlaid> {
        if family >= tree::POOL_FAMILIES {
            return Err(Unlaid::Cap);
        }
        let need = (family / CHUNK_FAMILIES + 1) as usize;
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

    /// Every family let go: the device that held the pool went, every
    /// brick with it, and nothing reads a tree until one lands again.
    pub fn clear(&mut self) {
        self.chunks = Vec::new();
    }

    /// Where word `k` of slot `slot`'s family lies, the heap's from 0 and
    /// the data's from 8: its chunk and its index in it.
    fn at(slot: u32, k: u32) -> (usize, usize) {
        let family = slot / 8;
        (
            (family / CHUNK_FAMILIES) as usize,
            ((family % CHUNK_FAMILIES) * FAMILY_WORDS + k + slot % 8) as usize,
        )
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
