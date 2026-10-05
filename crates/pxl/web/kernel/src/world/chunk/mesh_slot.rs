// pxl-kernel/world/chunk/mesh_slot.rs

//! Mesh slots in the renderer's arena (pxl-render gpu/arena.rs). A slot is
//! room, not an address and never the only copy: any tier takes any free
//! slot of its pool and every mesh can be rebuilt from the records.
//!
//! Three pools: the leaves' own tier (six-word boxes, a group of its
//! own, since a group is one format), the coarse tiers (four-word boxes,
//! shared groups), and the cover (one slot per possible root). A tier with
//! more boxes than a slot fills several, chained. A mesh is never cut to a
//! slot, which left steep ground missing (2026-09-26), nor baked coarser to
//! fit, which was a second quality policy.

use pxl_render::look::globe::MERCATOR_RADIUS_U;
use pxl_render::mesh::chunk_mesher::{COARSE_BOX_WORDS, FINE_BOX_WORDS};
use pxl_spec::space::pyramid::{CHUNK_CELLS, LEAF_Z, TOP_Z};

use crate::param::{ARENA_COARSE_GROUPS, ARENA_GROUP_SLOTS};
use crate::world::pyramid::domain::cover_pieces;
use pxl_spec::space::zoom::Zoom;

const SLOTS: u32 = ARENA_GROUP_SLOTS;
const COARSE_GROUPS: u32 = ARENA_COARSE_GROUPS;

/// One box per column: ground is a heightfield, so a column is one run per chunk.
pub const FINE_BOXES: u32 = CHUNK_CELLS * CHUNK_CELLS;
pub const COARSE_BOXES: u32 = crate::param::COARSE_BOXES;

/// Most slots a coarse-pool tier can fill: at step two a chunk bakes to at
/// most 2·32³ boxes (each cell plus the floor under it). A bound from the
/// representation, which `RESERVE` keeps free for one coarsening.
pub const COARSE_TIER_SLOTS: u32 = (2 * (CHUNK_CELLS / 2).pow(3)).div_ceil(COARSE_BOXES);

pub const POOLS: usize = 3;

/// At its coarsest a chunk is one cell: its box and the floor under it.
pub const COVER_BOXES: u32 = 2;

/// The most cover pieces over any top. Exact: each root holds one slot at
/// most, so a root the cover lacks always finds one free.
pub const COVER_SLOTS: u32 = {
    let (mut most, mut top) = (0, TOP_Z);
    while top <= LEAF_Z {
        let n = cover_pieces(top, MERCATOR_RADIUS_U);
        if n > most {
            most = n;
        }
        top += 1;
    }
    most
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pool {
    Fine = 0,
    Coarse = 1,
    Cover = 2,
}

impl Pool {
    pub const fn of(zoom: Zoom, step: u32) -> Pool {
        if step == CHUNK_CELLS {
            Pool::Cover
        } else if zoom == LEAF_Z && step == 1 {
            Pool::Fine
        } else {
            Pool::Coarse
        }
    }

    pub const fn slots(self) -> u32 {
        match self {
            Pool::Fine => SLOTS,
            Pool::Coarse => COARSE_GROUPS * SLOTS,
            Pool::Cover => COVER_SLOTS,
        }
    }

    const fn boxes(self) -> u32 {
        match self {
            Pool::Fine => FINE_BOXES,
            Pool::Coarse => COARSE_BOXES,
            Pool::Cover => COVER_BOXES,
        }
    }

    const fn first(self) -> usize {
        match self {
            Pool::Coarse => 0,
            Pool::Fine => Pool::Coarse.slots() as usize,
            Pool::Cover => (Pool::Coarse.slots() + Pool::Fine.slots()) as usize,
        }
    }

    pub fn range(self) -> core::ops::Range<usize> {
        self.first()..self.first() + self.slots() as usize
    }

    pub const fn at(index: usize) -> Pool {
        if index < Pool::Fine.first() {
            Pool::Coarse
        } else if index < Pool::Cover.first() {
            Pool::Fine
        } else {
            Pool::Cover
        }
    }
}

pub const ARENA: [u32; POOLS] = [Pool::Fine.slots(), Pool::Coarse.slots(), Pool::Cover.slots()];

pub const fn capacity(zoom: Zoom, step: u32) -> u32 {
    Pool::of(zoom, step).boxes()
}

pub const GROUPS: u32 = COARSE_GROUPS + 2;
const FINE_GROUP: u32 = COARSE_GROUPS;
const COVER_GROUP: u32 = COARSE_GROUPS + 1;
pub const OCCUPANCY: usize = (Pool::Coarse.slots() + Pool::Fine.slots() + Pool::Cover.slots()) as usize;

// WebGPU's default ceiling on one storage binding.
const _: () = assert!(group_bytes_of(0) as u64 <= 128 << 20, "an arena group past a storage binding's limit");
const _: () = assert!(group_bytes_of(FINE_GROUP) as u64 <= 128 << 20, "the leaves' group past a storage binding's limit");
const _: () = assert!(group_bytes_of(COVER_GROUP) as u64 <= 128 << 20, "the cover's group past a storage binding's limit");

const fn pool_of_group(group: u32) -> Pool {
    if group == FINE_GROUP {
        Pool::Fine
    } else if group == COVER_GROUP {
        Pool::Cover
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
    let slots = if group < COARSE_GROUPS { SLOTS } else { pool.slots() };
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
        Pool::Coarse => (n / SLOTS, n % SLOTS),
        Pool::Fine => (FINE_GROUP, n),
        Pool::Cover => (COVER_GROUP, n),
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
