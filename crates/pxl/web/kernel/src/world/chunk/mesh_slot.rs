// pxl-kernel/world/chunk/mesh_slot.rs

//! The render residency: where a drawn piece's mesh lies on the device, a
//! slot in the arena of fixed buffers the renderer keeps (pxl-render
//! gpu/arena.rs), and how many slots there are.
//!
//! Two contracts meet a mesh, and this is the second. The workspace is where
//! a mesh is made — one bake open at a time, in `.fixed.bake`
//! (world/chunk/build.rs `BAKE_BYTES`) — and it holds nothing across frames:
//! what it made is handed to a slot the moment it is done. The slots are
//! what is kept, and they are derived: every mesh in one is made again from
//! the records whenever it is asked for, so a slot is room and never the
//! only copy of anything. The workspace is sized to hold any mesh a slot
//! can; past that the two share nothing.
//!
//! A slot is room, not an address. Any tier may take any free slot of its
//! pool (world/chunk/table.rs), and keeps it until it is no longer drawn.
//! Slots are never taken from a mesh that is drawn: what a view needs past
//! the arena is drawn coarser by the quality policy (world/quality.rs), and
//! what is waiting for a slot waits with the coarser piece it would replace
//! still on the screen (world/chunk/frontier.rs).
//!
//! Two pools, because a group is one storage binding and a binding holds
//! 128 MiB. The leaves' own tier — the one drawn at their cells — has a
//! group to itself, since its box is six words where every other is four and
//! a group is one format throughout; the coarse tiers share
//! `ARENA_COARSE_GROUPS` groups between them, any tier of any level in any
//! of them.
//!
//! A slot holds a number of boxes, and a tier with more fills as many slots
//! as it takes, any free ones of its pool, chained in the order its boxes
//! are written (world/chunk/table.rs `Meshes::next`). What a tier is drawn
//! in is the quality's to choose and no slot's: a mesh is never cut to a
//! slot, which left ground simply missing — the chunk's last rows, wherever
//! steep ground made more boxes than the slot held, with its decor standing
//! over nothing (measured 2026-09-26) — and never baked coarser than asked
//! to fit one, which was a second quality policy, leaking past a cell where
//! placed objects are dense. Every slot a tier fills is held for it at once,
//! when its bake knows its size, or none is and the bake is thrown away
//! (build.rs `BuildEvent::Reserve`); how many it fills is then counted by
//! the frontier and the quality policy, which alone decides what the arena
//! has no room for. The allocation knows slots and nothing of why.
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
//! `coarse_peak`, and how many filled more than one slot as its
//! `slot chained`. The coarse tier asks for more than a leaf's own: nothing of
//! it merges, since a box shrunk about its cell's center touches nothing
//! (issue/20260923-a-coarse-cell-is-isotropic.md).

use pxl_render::look::lod::COARSEST;
use pxl_render::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_spec::space::pyramid::CHUNK_CELLS;

use crate::param::{ARENA_COARSE_GROUPS, ARENA_GROUP_SLOTS};

/// Slots a group holds (param.rs).
const SLOTS: u32 = ARENA_GROUP_SLOTS;
/// Groups the coarse tiers share (param.rs).
const COARSE_GROUPS: u32 = ARENA_COARSE_GROUPS;

/// Boxes a leaf's first tier holds, the one tier drawn at a leaf's own cells:
/// one a column. The ground is a heightfield, so a column of it is one run
/// in a chunk, and a chunk is a cube that cuts it to its heights
/// (world/pyramid/chunk.rs); a box is at most a run (pxl-render
/// mesh/chunk_mesher.rs). A chunk whose placed objects make it more fills
/// more of them.
pub const FINE_BOXES: u32 = CHUNK_CELLS * CHUNK_CELLS;
/// Boxes every other tier holds (param.rs). It is not a bound the content
/// keeps, and needs not be one: a coarse cell at step `s` is a cube `s`
/// cells a side, holding at most its own box and the floor laid under it, so
/// a chunk — 64 cells a side in all three — bakes to at most 2·(64/s)³
/// boxes, and a mesh past one slot fills more. The coarsest step, one cell
/// a chunk, fills one, and the cover is drawn at it everywhere
/// (world/chunk/frontier.rs `drawn_cube`), so the cover is a slot a piece
/// by its representation, as the arena's sizing counts it
/// (world/quality.rs).
pub const COARSE_BOXES: u32 = crate::param::COARSE_BOXES;

const _: () = assert!(2 * (CHUNK_CELLS / COARSEST).pow(3) <= COARSE_BOXES, "a chunk at its coarsest step past what a coarse slot holds");

/// The most slots a tier of the coarse pool fills, whatever it holds: every
/// tier there is drawn at a step of two or more — a level's cells averaged
/// 2 × 2 (world/chunk/frontier.rs `drawn_cube`), or a z16 chunk drawn
/// coarser — and a coarse cell is at most its own box and the floor laid
/// under it, so a chunk at step two bakes to at most 2·32³ boxes. A bound
/// from the representation, not from any mesh met so far: what the
/// frontier keeps free so a coarsening, whose piece is over z16 and so of
/// this pool, can always be built (frontier.rs `RESERVE`).
pub const COARSE_TIER_SLOTS: u32 = (2 * (CHUNK_CELLS / 2).pow(3)).div_ceil(COARSE_BOXES);

/// The pools a tier's slot is taken from, as an index.
pub const POOLS: usize = 2;

/// Which of the arena's pools a tier takes its slot from: the leaves' own
/// tier, or any other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pool {
    Fine = 0,
    Coarse = 1,
}

impl Pool {
    /// The pool a tier of step `step` on a chunk of `level` takes.
    pub const fn of(level: u32, step: u32) -> Pool {
        if level == 0 && step == 1 {
            Pool::Fine
        } else {
            Pool::Coarse
        }
    }

    /// Slots it holds.
    pub const fn slots(self) -> u32 {
        match self {
            Pool::Fine => SLOTS,
            Pool::Coarse => COARSE_GROUPS * SLOTS,
        }
    }

    /// Which slots are its.
    pub fn range(self) -> core::ops::Range<usize> {
        match self {
            Pool::Fine => (FINE_GROUP * SLOTS) as usize..((FINE_GROUP + 1) * SLOTS) as usize,
            Pool::Coarse => 0..(COARSE_GROUPS * SLOTS) as usize,
        }
    }

    /// The pool a slot is in.
    pub const fn at(index: usize) -> Pool {
        if index as u32 / SLOTS == FINE_GROUP {
            Pool::Fine
        } else {
            Pool::Coarse
        }
    }
}

/// Every pool's slots, by `Pool` as an index: the arena the quality policy
/// fits a view's frontier into.
pub const ARENA: [u32; POOLS] = [Pool::Fine.slots(), Pool::Coarse.slots()];

/// Boxes a tier of step `step` on a chunk of `level` may have: its pool's.
pub const fn capacity(level: u32, step: u32) -> u32 {
    match Pool::of(level, step) {
        Pool::Fine => FINE_BOXES,
        Pool::Coarse => COARSE_BOXES,
    }
}

/// Groups the arena holds: the coarse tiers', and the leaves' own tier
/// beside them.
pub const GROUPS: u32 = COARSE_GROUPS + 1;
/// The group the leaves' own tier lies in.
const FINE_GROUP: u32 = COARSE_GROUPS;
/// Slots across every group, for whoever tracks who holds each.
pub const OCCUPANCY: usize = (GROUPS * SLOTS) as usize;

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

/// Words a slot's surface lane holds: a sample at the center of each of its
/// tier's blocks, CHUNK_CELLS a side for the leaves' own tier, which draws
/// its level's cells, and half that for every other, which draws no finer
/// than its level's cells averaged 2 × 2 (world/chunk/frontier.rs
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
