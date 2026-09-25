// pxl-kernel/world/chunk/mesh_slot.rs

//! Where a chunk's mesh lies on the device: a slot in the arena of fixed
//! buffers the renderer keeps (pxl-render gpu/arena.rs).
//!
//! A slot is room, not an address. Any tier may take any slot of its pool,
//! and which one it takes is the scheduler's to decide
//! (world/chunk/table.rs): a free one, else one whose holder is not drawing
//! from it. A chunk's mesh keeps the slot it took until something takes it
//! back, so a mesh outlives its page's record — it is drawn until something
//! covers its ground — and there is nothing to free.
//!
//! A slot named by the chunk's coordinate — reduced modulo the drawn ring's
//! axis in chunks, within a group for its level modulo the levels a view
//! holds — makes a level's chunks addressable *only* inside its own drawn
//! ring, and so decides what can be drawn at all: `select` cannot descend
//! past a page it does not hold, because the chunks under it lie outside the
//! ring below and two of them land in one slot. Of the nine pages a top ring
//! has, four fold from what is held and five do not, and every one of those
//! five has one or two of its four children resident: nearly half the ground
//! that shows as nothing is data the client already has and cannot address.
//! Cesium's answer to the same thing is `skipLevelOfDetail`: a tile whose
//! content is missing is refined through rather than rendered, and the
//! loaded descendants are drawn.
//!
//! Two pools, because a group is one storage binding and a binding holds
//! 128 MiB. The leaves' own tier — the one drawn at their cells — has a
//! group to itself, since its box is six words where every other is four and
//! a group is one format throughout; the coarse tiers share
//! COARSE_GROUPS groups between them, any tier of any level in any of them.
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
//! What each tier was measured to ask for is on the constant sized against
//! it, and nowhere else — two copies of one measurement is how this file came
//! to carry two different pairs of numbers for it (2026-09-23). What the
//! measurement showed, and the constants cannot say, is that the coarse tier
//! asks for *more* than the leaf's own: nothing of it merges, since a box
//! shrunk about its cell's centre touches nothing
//! (issue/20260923-a-coarse-cell-is-isotropic.md). Either is read live as the
//! ledger's `work fine_peak` and `coarse_peak`, through a zoom out and back.

use crate::world::pyramid::domain::{LEVEL_CHUNKS, VIEW_CHUNKS};
use pxl_render::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};

/// Slots a group holds: the most chunks one level draws, which is what the
/// leaves' own tier needs, since only z16 draws it.
const SLOTS: u32 = LEVEL_CHUNKS;
/// Boxes a leaf's first tier holds, the one tier drawn at a leaf's own cells:
/// the 2,294 the measurement asked for and a fifth again.
pub const FINE_BOXES: u32 = 2730;
/// Boxes every other tier holds: the 3,769 it asked for and a fifth again.
pub const COARSE_BOXES: u32 = 4551;

/// The largest mesh one bake may write before it is cut to its slot: past
/// every tier measured, so what a bake asks for always fits.
pub const BAKE_BYTES: u32 = 1 << 20;
/// Groups the coarse tiers share: a slot for every chunk a view draws, each
/// drawn in one coarse tier, and half as many again for what a change of view
/// holds on to while it lasts — a dissolve's other tier, a stale chunk
/// drawn until what replaced it is whole. Past that a mesh waits for a slot
/// rather than take one from ground on the screen (world/chunk/table.rs
/// `claim`).
const COARSE_GROUPS: u32 = (VIEW_CHUNKS * 3 / 2).div_ceil(SLOTS);
/// Groups the arena holds: the coarse tiers', and the leaves' own tier
/// beside them.
pub const GROUPS: u32 = COARSE_GROUPS + 1;
/// The group the leaves' own tier lies in.
const FINE_GROUP: u32 = COARSE_GROUPS;
/// Slots across every group, for whoever tracks who holds each.
pub const OCCUPANCY: usize = (GROUPS * SLOTS) as usize;

const _: () = assert!(
    FINE_BOXES * FINE_BOX_WORDS * 4 <= BAKE_BYTES && COARSE_BOXES * COARSE_BOX_WORDS * 4 <= BAKE_BYTES,
    "a slot past what one bake can write"
);
// WebGPU's default ceiling on one storage binding.
const _: () = assert!(group_bytes_of(0) as u64 <= 128 << 20, "an arena group past a storage binding's limit");
const _: () = assert!(group_bytes_of(FINE_GROUP) as u64 <= 128 << 20, "the leaves' group past a storage binding's limit");

/// Whether group `group` holds the leaves' own box format, which is what the
/// renderer draws it at: the two formats are two pipelines and two vertex
/// entry points, so a draw says which by the one it is listed under.
pub const fn group_is_fine(group: u32) -> bool {
    group == FINE_GROUP
}

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

/// Which slots a tier of step `step` on a chunk of `level` may take. The
/// leaves' own tier has its own pool; every other tier shares one.
pub fn pool(level: u32, step: u32) -> core::ops::Range<usize> {
    if level == 0 && step == 1 {
        (FINE_GROUP * SLOTS) as usize..((FINE_GROUP + 1) * SLOTS) as usize
    } else {
        0..(COARSE_GROUPS * SLOTS) as usize
    }
}

/// The slot an index names: which group's arena it is in, where in it, and
/// how many boxes it holds.
pub fn slot_at(index: usize) -> MeshSlot {
    let (group, residue) = (index as u32 / SLOTS, index as u32 % SLOTS);
    let capacity = if group == FINE_GROUP { FINE_BOXES } else { COARSE_BOXES };
    MeshSlot {
        group,
        first_box: residue * capacity,
        capacity,
        index,
    }
}

#[cfg(test)]
#[path = "mesh_slot.test.rs"]
mod test;
