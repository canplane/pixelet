// pxl-kernel/world/chunk/mesh_slot.rs

//! Where a chunk's mesh lies on the device: a slot decided by the chunk's
//! own coordinate, the way a terrain page's record is (layer/terrain/fixed.rs),
//! in the arena of fixed buffers the renderer keeps (pxl-render gpu/arena.rs).
//!
//! A slot is the chunk's coordinate at its level reduced modulo the drawn
//! ring's axis in chunks, within a group of slots for its level reduced
//! modulo the window's width: two chunks of one level share a slot only when
//! they lie a whole drawn ring apart, and two levels only when they lie a
//! whole window apart. The leaves' own tier — the one drawn at their cells —
//! has a group to itself rather than sharing the leaves' group, because a
//! group is one storage binding and a binding holds 128 MiB: with both in
//! one, what either could hold was what the other left. A chunk's mesh takes its slot and keeps it
//! until another chunk of the same residue takes it, which the scheduler
//! lets happen only once the one holding it is no longer drawn
//! (world/chunk/table.rs). So a mesh outlives its page's record — it is
//! drawn until something covers its ground — and there is nothing to free.
//!
//! A slot holds as many boxes as its tier has been measured to need, and a
//! mesh past that is cut to it — ground that is simply missing, which the
//! ledger's `slot cut` says outright.
//!
//! A slot's cost is its box's words (pxl-render mesh/chunk_mesher.rs), and
//! the two tiers do not weigh the same: a leaf's box is six words and every
//! other four, because a box that merges cells carries no texture taps and
//! is never drawn near enough to need a leaf's resolution across a cell. The
//! arena is the largest thing the device holds, so what a box weighs there
//! is what every measurement below is multiplied by. A group is one format
//! throughout, which is what lets a slot be `residue × capacity`.
//!
//! Measured through a zoom out and back over Seoul (2026-09-23, the ledger's
//! `work fine_boxes` and `coarse_boxes`), under the coarse rule where a cell
//! is its fill on every axis: a leaf's own tier asked 1,729 boxes and a
//! coarse tier 2,198. The coarse tier is the larger of the two because
//! nothing of it merges — a box shrunk about its cell's centre touches
//! nothing (issue/20260923-a-coarse-cell-is-isotropic.md).

use pxl_render::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};

use crate::layer::terrain::level::{RENDER_AXIS, WINDOW_LEVELS};
use crate::world::chunk::range::PAGE_CHUNK_AXIS;

/// Chunks along a level's drawn ring, the axis a slot is reduced modulo.
pub const AXIS: u32 = RENDER_AXIS * PAGE_CHUNK_AXIS;
const SLOTS: u32 = AXIS * AXIS;
/// Boxes a leaf's first tier holds, the one tier drawn at a leaf's own cells:
/// the 2,294 the measurement asked for and a fifth again.
pub const FINE_BOXES: u32 = 2730;
/// Boxes every other tier holds: the 3,769 it asked for and a fifth again.
pub const COARSE_BOXES: u32 = 4551;

/// The largest mesh one bake may write before it is cut to its slot: past
/// every tier measured, so what a bake asks for always fits.
pub const BAKE_BYTES: u32 = 1 << 20;
/// Groups the arena holds: one a level of the window, and the leaves' own
/// tier beside them.
pub const GROUPS: u32 = WINDOW_LEVELS + 1;
/// The group the leaves' own tier lies in.
const FINE_GROUP: u32 = WINDOW_LEVELS;
/// Slots across every group, for whoever tracks who holds each.
pub const OCCUPANCY: usize = (GROUPS * SLOTS) as usize;

const _: () = assert!(
    FINE_BOXES * FINE_BOX_WORDS * 4 <= BAKE_BYTES && COARSE_BOXES * COARSE_BOX_WORDS * 4 <= BAKE_BYTES,
    "a slot past what one bake can write"
);
// WebGPU's default ceiling on one storage binding.
const _: () = assert!(group_bytes_of(0) as u64 <= 128 << 20, "an arena group past a storage binding's limit");
const _: () = assert!(group_bytes_of(FINE_GROUP) as u64 <= 128 << 20, "the leaves' group past a storage binding's limit");

/// Words a box of group `group` takes.
pub const fn group_box_words(group: u32) -> u32 {
    if group == FINE_GROUP {
        FINE_BOX_WORDS
    } else {
        COARSE_BOX_WORDS
    }
}

/// Boxes group `group` holds: a coarse tier's slots, or the leaves' own.
pub const fn group_boxes(group: u32) -> u32 {
    if group == FINE_GROUP {
        SLOTS * FINE_BOXES
    } else {
        SLOTS * COARSE_BOXES
    }
}

const fn group_bytes_of(group: u32) -> u32 {
    group_boxes(group) * group_box_words(group) * 4
}

/// Every group's bytes, the arena's to make.
pub fn group_bytes() -> [u32; GROUPS as usize] {
    core::array::from_fn(|g| group_bytes_of(g as u32))
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
    let fine = level == 0 && step == 1;
    let group = if fine { FINE_GROUP } else { level % WINDOW_LEVELS };
    let residue = cy.rem_euclid(AXIS as i64) as u32 * AXIS + cx.rem_euclid(AXIS as i64) as u32;
    let capacity = if fine { FINE_BOXES } else { COARSE_BOXES };
    MeshSlot {
        group,
        first_box: residue * capacity,
        capacity,
        index: (group * SLOTS + residue) as usize,
    }
}

#[cfg(test)]
#[path = "mesh_slot.test.rs"]
mod test;
