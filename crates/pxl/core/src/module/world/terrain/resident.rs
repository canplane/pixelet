// pxl_core/module/world/terrain/resident.rs

//! A terrain page, resident in the core memory.
//!
//! The kernel copies a fetched page's decoded lanes into blocks of the page
//! section and drops the body: one memory holds the core's data. Per
//! lane, not per page — buddy blocks are powers of two, and a whole page would
//! round up to 8 MiB, while each channel is exactly one 1 MiB block.
//!
//! **Release is quarantined.** Readers of a page are the chunks in its render
//! radius; an evicted page is past the cleanup radius, one ring further out, so
//! none should remain. The case that can still hold one is a jump: the page is
//! evicted in the same pass that moves the view, and its chunks leave after. A
//! released block is only overwritten when something acquires it again, so
//! holding the release back for a second covers that with room to spare.

use pxl_shared::spec::space::pvf3::ivec::IVec;
use pxl_shared::spec::space::xy::TO_XY;

use crate::global::{now, Global};
use crate::kernel::kernel;
use crate::memory::PAGE_POOL;

pub const QUARANTINE_MS: f64 = 1000.0;

/// A block of the page section: its address and size.
pub type Block = (usize, u32);

struct Resident {
    /// The blocks each resident page owns. The shared ocean page is not in
    /// here: every open-ocean page aliases one decode, so no single page may
    /// free it.
    owned: Vec<(IVec, Vec<Block>)>,
    /// Blocks released, and when their quarantine ends.
    due: Vec<(f64, Block)>,
}

static RESIDENT: Global<Resident> = Global::new();

fn resident() -> &'static mut Resident {
    RESIDENT.get(|| Resident { owned: Vec::new(), due: Vec::new() })
}

/// Records the blocks a page's lanes were copied into. `own` is false for a
/// page that is shared rather than held by one tile — it is never released.
pub fn into_core(page_idx: IVec, blocks: Vec<Block>, own: bool) {
    if !own {
        return;
    }
    let key = TO_XY(page_idx);
    let r = resident();
    r.owned.retain(|(p, _)| *p != key);
    r.owned.push((key, blocks));
}

/// Hands a page's blocks back once the quarantine has passed. A page it does
/// not own — the shared ocean page — is left alone.
pub fn release_terrain(page_idx: IVec, quarantine_ms: f64) {
    let key = TO_XY(page_idx);
    let r = resident();
    let Some(pos) = r.owned.iter().position(|(p, _)| *p == key) else { return };
    let (_, blocks) = r.owned.swap_remove(pos);
    let due = now() + quarantine_ms;
    r.due.extend(blocks.into_iter().map(|b| (due, b)));
}

/// The blocks whose quarantine has passed.
pub fn take_due_releases() -> Vec<Block> {
    let r = resident();
    let t = now();
    let (due, waiting): (Vec<_>, Vec<_>) = r.due.drain(..).partition(|(at, _)| *at <= t);
    r.due = waiting;
    due.into_iter().map(|(_, b)| b).collect()
}

/// Gives the page section back the blocks whose quarantine has passed.
pub fn release_due_terrain() {
    let Some(pool) = kernel().pools.get_mut(PAGE_POOL as usize) else { return };
    let t = now();
    resident().due.retain(|&(due, (at, bytes))| {
        if due > t {
            return true;
        }
        pool.release(at, bytes);
        false
    });
}

#[cfg(test)]
#[path = "resident.test.rs"]
mod test;
