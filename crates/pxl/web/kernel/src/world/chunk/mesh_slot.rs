// pxl-kernel/world/chunk/mesh_slot.rs

//! Where a chunk's mesh lies on the device: a slot decided by the chunk's
//! own coordinate, the way a terrain page's record is (layer/terrain/fixed.rs),
//! in the arena of fixed buffers the renderer keeps (pxl-render gpu/arena.rs).
//!
//! A slot is the chunk's coordinate at its level reduced modulo the drawn
//! ring's axis in chunks, within a group of slots for its level reduced
//! modulo the window's width, a tier a slot: two chunks of one level share a
//! slot only when they lie a whole drawn ring apart, and two levels only when
//! they lie a whole window apart. A chunk's mesh takes its slot and keeps it
//! until another chunk of the same residue takes it, which the scheduler
//! lets happen only once the one holding it is no longer drawn
//! (world/chunk/table.rs). So a mesh outlives its page's record — it is
//! drawn until something covers its ground — and there is nothing to free.
//!
//! A slot holds as many boxes as its tier has been measured to need, and a
//! mesh past that is cut to it — ground that is simply missing, which the
//! ledger's `slot cut` says outright.
//!
//! Measured through a zoom out and back over Seoul (2026-09-23, the ledger's
//! `work fine_boxes` and `coarse_boxes`), under the coarse rule where a cell
//! is its fill on every axis: a leaf's own tier asked 1,729 boxes and a
//! coarse tier 2,198. The coarse tier is the larger of the two because
//! nothing of it merges — a box shrunk about its cell's centre touches
//! nothing (issue/20260923-a-coarse-cell-is-isotropic.md).

use pxl_render::mesh::chunk_mesher::BOX_WORDS;

use crate::layer::terrain::level::{RENDER_AXIS, WINDOW_LEVELS};
use crate::world::chunk::range::PAGE_CHUNK_AXIS;

/// Chunks along a level's drawn ring, the axis a slot is reduced modulo.
pub const AXIS: u32 = RENDER_AXIS * PAGE_CHUNK_AXIS;
const SLOTS: u32 = AXIS * AXIS;
/// Boxes a leaf's first tier holds, the one tier drawn at a leaf's own cells.
pub const FINE_BOXES: u32 = 3640;
/// Boxes every other tier holds: what the measurement asked for and a
/// quarter again.
pub const COARSE_BOXES: u32 = 2730;
const BOX_BYTES: u32 = BOX_WORDS * 4;
/// The largest mesh one bake may write before it is cut to its slot: past
/// every tier measured, so what a bake asks for always fits.
pub const BAKE_BYTES: u32 = 1 << 20;
/// Slots across every group and tier, for whoever tracks who holds each.
pub const OCCUPANCY: usize = (WINDOW_LEVELS * SLOTS * 2) as usize;

const _: () = assert!(FINE_BOXES * BOX_BYTES <= BAKE_BYTES && COARSE_BOXES * BOX_BYTES <= BAKE_BYTES, "a slot past what one bake can write");
// WebGPU's default ceiling on one storage binding.
const _: () = assert!(group_boxes(0) as u64 * BOX_BYTES as u64 <= 128 << 20, "an arena group past a storage binding's limit");

/// Boxes group `group` holds: the leaves' group has their first tier's slots
/// besides the coarse ones every group has.
pub const fn group_boxes(group: u32) -> u32 {
    if group == 0 {
        SLOTS * (FINE_BOXES + COARSE_BOXES)
    } else {
        SLOTS * COARSE_BOXES
    }
}

/// Every group's bytes, the arena's to make.
pub fn group_bytes() -> [u32; WINDOW_LEVELS as usize] {
    core::array::from_fn(|g| group_boxes(g as u32) * BOX_BYTES)
}

/// A tier's place: its group, its first box there, how many boxes it may
/// hold, and its index among every slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshSlot {
    pub group: u32,
    pub first_box: u32,
    pub capacity: u32,
    pub index: usize,
}

/// The slot of the tier of step `step`, in the chunk's own cells, of the chunk
/// at `(cx, cy)` in chunks of level `level`.
pub fn slot_of(level: u32, cx: i64, cy: i64, step: u32) -> MeshSlot {
    let group = level % WINDOW_LEVELS;
    let residue = cy.rem_euclid(AXIS as i64) as u32 * AXIS + cx.rem_euclid(AXIS as i64) as u32;
    let fine = level == 0 && step == 1;
    let first_box = match (fine, group) {
        (true, _) => residue * FINE_BOXES,
        (false, 0) => SLOTS * FINE_BOXES + residue * COARSE_BOXES,
        (false, _) => residue * COARSE_BOXES,
    };
    MeshSlot {
        group,
        first_box,
        capacity: if fine { FINE_BOXES } else { COARSE_BOXES },
        index: ((group * 2 + fine as u32) * SLOTS + residue) as usize,
    }
}

#[cfg(test)]
#[path = "mesh_slot.test.rs"]
mod test;
