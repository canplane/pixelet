// pxl-kernel/world/chunk/mesh_slot.rs

//! Mesh slots in the renderer's arena (pxl-render gpu/arena.rs). A slot is
//! room, not an address and never the only copy: any tier takes any free
//! slot of its pool and every mesh can be rebuilt from the records.
//!
//! Two pools: chunks of a leaf's cells or finer (six-word textured boxes, a
//! group of its own, since a group is one format) and coarser chunks
//! (four-word boxes, shared groups). A tier with more boxes than a slot fills several, chained. A mesh is never cut to a
//! slot, which left steep ground missing (2026-09-26), nor baked coarser to
//! fit, which was a second quality policy.

use pxl_render::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_spec::bake::constant::CELL;
use pxl_spec::space::pyramid::CHUNK_CELLS;

use crate::param::{ARENA_COARSE_GROUPS, COARSE_POOL_BYTES, FINE_POOL_BYTES};
use pxl_spec::space::zoom::Zoom;

const COARSE_GROUPS: u32 = ARENA_COARSE_GROUPS;

/// One box per column: ground is a heightfield, so a column is one run per chunk.
pub const FINE_BOXES: u32 = CHUNK_CELLS * CHUNK_CELLS;
/// A coarse chunk's cells at the boxes each draws.
pub const COARSE_BOXES: u32 = ((CHUNK_CELLS * CHUNK_CELLS) as f64 * crate::param::COARSE_BOXES_PER_CELL) as u32;

/// Slots each pool's bytes hold.
const FINE_SLOTS: u32 = FINE_POOL_BYTES / (FINE_BOXES * FINE_BOX_WORDS * 4);
const GROUP_SLOTS: u32 = COARSE_POOL_BYTES / COARSE_GROUPS / (COARSE_BOXES * COARSE_BOX_WORDS * 4);

/// Most slots a coarse-pool tier can fill: a chunk bakes to at most two boxes
/// a cell (each cell plus the floor under it). A bound from the
/// representation, which `RESERVE` keeps free for one coarsening.
pub const COARSE_TIER_SLOTS: u32 = (2 * CHUNK_CELLS.pow(3)).div_ceil(COARSE_BOXES);

pub const POOLS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pool {
    Fine = 0,
    Coarse = 1,
}

impl Pool {
    /// A tier drawing cells of `cell_zoom`: textured, a leaf's or finer; or coarser.
    pub const fn of(cell_zoom: Zoom) -> Pool {
        if cell_zoom >= CELL {
            Pool::Fine
        } else {
            Pool::Coarse
        }
    }

    pub const fn slots(self) -> u32 {
        match self {
            Pool::Fine => FINE_SLOTS,
            Pool::Coarse => COARSE_GROUPS * GROUP_SLOTS,
        }
    }

    const fn boxes(self) -> u32 {
        match self {
            Pool::Fine => FINE_BOXES,
            Pool::Coarse => COARSE_BOXES,
        }
    }

    const fn first(self) -> usize {
        match self {
            Pool::Coarse => 0,
            Pool::Fine => Pool::Coarse.slots() as usize,
        }
    }

    pub fn range(self) -> core::ops::Range<usize> {
        self.first()..self.first() + self.slots() as usize
    }

    pub const fn at(index: usize) -> Pool {
        if index < Pool::Fine.first() {
            Pool::Coarse
        } else {
            Pool::Fine
        }
    }
}

pub const ARENA: [u32; POOLS] = [Pool::Fine.slots(), Pool::Coarse.slots()];

pub const fn capacity(cell_zoom: Zoom) -> u32 {
    Pool::of(cell_zoom).boxes()
}

pub const GROUPS: u32 = COARSE_GROUPS + 1;
const FINE_GROUP: u32 = COARSE_GROUPS;
pub const OCCUPANCY: usize = (Pool::Coarse.slots() + Pool::Fine.slots()) as usize;

// WebGPU's default ceiling on one storage binding.
const _: () = assert!(group_bytes_of(0) as u64 <= 128 << 20, "an arena group past a storage binding's limit");
const _: () = assert!(group_bytes_of(FINE_GROUP) as u64 <= 128 << 20, "the leaves' group past a storage binding's limit");

const fn pool_of_group(group: u32) -> Pool {
    if group == FINE_GROUP {
        Pool::Fine
    } else {
        Pool::Coarse
    }
}

/// The two box formats are two pipelines, so a draw says which by the group it is in.
pub const fn group_is_fine(group: u32) -> bool {
    group == FINE_GROUP
}

pub const fn group_box_words(group: u32) -> u32 {
    if group == FINE_GROUP {
        FINE_BOX_WORDS
    } else {
        COARSE_BOX_WORDS
    }
}

pub const fn group_boxes(group: u32) -> u32 {
    let pool = pool_of_group(group);
    let slots = if group < COARSE_GROUPS { GROUP_SLOTS } else { pool.slots() };
    slots * pool.boxes()
}

const fn group_bytes_of(group: u32) -> u32 {
    group_boxes(group) * group_box_words(group) * 4
}

pub fn group_bytes() -> [u32; GROUPS as usize] {
    core::array::from_fn(|g| group_bytes_of(g as u32))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshSlot {
    pub group: u32,
    pub first_box: u32,
    pub capacity: u32,
    pub index: usize,
}

pub fn slot_at(index: usize) -> MeshSlot {
    let pool = Pool::at(index);
    let n = (index - pool.first()) as u32;
    let (group, residue) = match pool {
        Pool::Coarse => (n / GROUP_SLOTS, n % GROUP_SLOTS),
        Pool::Fine => (FINE_GROUP, n),
    };
    let capacity = pool.boxes();
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
