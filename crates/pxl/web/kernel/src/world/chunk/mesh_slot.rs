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
//! A slot holds a number of boxes derived from the chunk's geometry, and a
//! mesh never goes past it: one that would is baked a step coarser instead
//! (world/chunk/build.rs), and a step always fits (`COARSE_BOXES`). Cut to
//! its slot instead, a mesh leaves ground simply missing: the chunk's last
//! rows, wherever steep ground makes more boxes than the slot holds, with its
//! decor standing over nothing (measured 2026-09-26).
//!
//! A slot's cost is its box's words (pxl-render mesh/chunk_mesher.rs), and
//! the two tiers do not weigh the same: a leaf's box is six words and every
//! other four, because a box that merges cells carries no texture taps and
//! is never drawn near enough to need a leaf's resolution across a cell. The
//! arena is the largest thing the device holds, so what a box weighs there
//! is what every measurement below is multiplied by. A group is one format
//! throughout, which is what lets a slot be `residue × capacity`.
//!
//! What each tier asks for is read live as the profiler's `work fine_peak` and
//! `coarse_peak`, and how many were baked a step coarser than asked as its
//! `slot coarser`. The coarse tier asks for more than a leaf's own: nothing of
//! it merges, since a box shrunk about its cell's center touches nothing
//! (issue/20260923-a-coarse-cell-is-isotropic.md).

use pxl_render::look::lod::COARSEST;
use pxl_render::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_spec::space::pyramid::CHUNK_CELLS;

/// Slots a group holds, and so the z16 chunks the leaves' own tier can draw
/// at once. A product number, said aloud: the arena is the largest thing the
/// device holds, and this is what it gives a view. It is the z16 chunks over
/// flat ground within where z16 ends, as a disc; the rule's own bound
/// (domain.rs `LEVEL_PIECES`, a ball, since steep ground or stacked objects
/// make several chunks a column) is some eighteen times it, which for the
/// leaves' tier alone is past a storage binding. A view past it waits for a
/// slot (world/chunk/table.rs `claim`) rather than take one from ground on
/// the screen.
const SLOTS: u32 = 330;
/// Boxes a leaf's first tier holds, the one tier drawn at a leaf's own cells:
/// one a column. The ground is a heightfield, so a column of it is one run
/// in a chunk, and a chunk is a cube that cuts it to its heights
/// (world/pyramid/chunk.rs); a box is at most a run (pxl-render
/// mesh/chunk_mesher.rs). A chunk whose placed objects make it more is baked
/// a step coarser, into the coarse pool.
pub const FINE_BOXES: u32 = CHUNK_CELLS * CHUNK_CELLS;
/// Boxes every other tier holds. A product number, said aloud: the room the
/// coarse pool gives a mesh, which is its size on the device. It is not a
/// bound the content keeps, and needs not be one: a coarse cell at step `s`
/// is a cube `s` cells a side, holding at most its own box and the floor laid
/// under it, so a chunk — 64 cells a side in all three — bakes to at most
/// 2·(64/s)³ boxes, and a mesh that does not fit is baked a step coarser
/// until it does. This fits at step 8 at the latest, and the coarsest step,
/// one cell a chunk, always.
pub const COARSE_BOXES: u32 = 4551;

const _: () = assert!(2 * (CHUNK_CELLS / COARSEST).pow(3) <= COARSE_BOXES, "a chunk at its coarsest step past what a coarse slot holds");

/// Boxes a tier of step `step` on a chunk of `level` may have: its pool's.
pub const fn capacity(level: u32, step: u32) -> u32 {
    if level == 0 && step == 1 {
        FINE_BOXES
    } else {
        COARSE_BOXES
    }
}

/// The largest mesh one bake may write: past what any slot holds, so a bake
/// is weighed against its slot before it could outgrow its block.
pub const BAKE_BYTES: u32 = 1 << 20;
/// Groups the coarse tiers share. A product number, said aloud: a slot for
/// each of some thousand chunks a view draws, each in one coarse tier, and
/// half as many again for what a change of view holds on to while it lasts —
/// a dissolve's other tier, a stale chunk drawn until what replaced it is
/// whole. The rule's bound (domain.rs `VIEW_PIECES`) with the same half
/// again is some sixty times it, gigabytes of boxes. Past it a mesh waits
/// for a slot rather than take one from ground on the screen
/// (world/chunk/table.rs `claim`).
const COARSE_GROUPS: u32 = 5;
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

/// Words a slot's surface lane holds: a sample at the center of each of its
/// tier's blocks, CHUNK_CELLS a side for the leaves' own tier, which draws
/// its level's cells, and half that for every other, which draws no finer
/// than its level's cells averaged 2 × 2 (world/chunk/table.rs
/// `drawn_cube`).
const fn surface_words(group: u32) -> u32 {
    let axis = if group == FINE_GROUP { CHUNK_CELLS } else { CHUNK_CELLS / 2 };
    axis * axis
}

/// `.fixed.surface`: every slot's surface lane, the coarse groups' then the
/// leaves'.
pub const SURFACE_BYTES: u32 = SLOTS * (COARSE_GROUPS * surface_words(0) + surface_words(FINE_GROUP)) * 4;

/// Where slot `index`'s surface lane lies in `.fixed.surface`, and the words
/// it holds.
pub const fn surface_of(index: usize) -> (u32, u32) {
    let (group, residue) = (index as u32 / SLOTS, index as u32 % SLOTS);
    ((group * SLOTS * surface_words(0) + residue * surface_words(group)) * 4, surface_words(group))
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
