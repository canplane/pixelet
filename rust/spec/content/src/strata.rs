// pxl-content/strata.rs

//! gpu-portable: the ground's strata, by depth under a column's face (after
//! Minecraft Beta's `replaceBlocksForBiome`): the look on top, a filler of
//! soil, sand or water, sandstone under sand, stone under all, down to the
//! bedrock; how high a cube may reach over the face; and past the service
//! area, the Earth's ground.

use pxl_engine::cube::ground;
use pxl_engine::ground::{face_height, leaf_cells, rock, UNKNOWN, VOID};
use pxl_engine::{Zoom, LEAF_CELL, SEA_LEVEL};

use crate::param::strata::{BEDROCK_DEPTH, FILLER_DEPTH, HEADROOM, SANDSTONE_DEPTH};
use crate::Pixelet;

// The palette entries the strata name, read into a word.
const SAND: u32 = ground::sand as u32;
const BEACH: u32 = ground::beach as u32;
const WATER: u32 = ground::water as u32;
const SOIL: u32 = ground::soil as u32;
const SANDSTONE: u32 = ground::sandstone as u32;
const STONE: u32 = ground::stone as u32;

/// The bedrock's top under a face at `face` leaf cells, in leaf cells: the
/// lowest a cube may reach there, every walk's, edit's and column's.
pub fn bedrock(face: i32) -> i32 {
    face.min(SEA_LEVEL) - BEDROCK_DEPTH
}

pub fn ceiling(face: i32) -> i32 {
    face.max(SEA_LEVEL) + HEADROOM
}

/// What the ground is in cell `z` of a column of `cell` whose palette index is `palette_idx`
/// and whose top is `top` cells: by depth under its top, `void` over it. A
/// column coarser than the leaves' is a summary of the faces under it, its
/// ground its face whole; the wall (`unknown`) is itself all through.
pub fn by_depth(cell: Zoom, palette_idx: u32, top: i32, z: i32) -> u32 {
    if z >= top {
        return VOID;
    }
    if cell < LEAF_CELL || palette_idx == UNKNOWN {
        return palette_idx;
    }
    let s = leaf_cells(cell);
    // Water's filler is water, as sand's is sand: under a face it is the
    // same cell.
    let (filler, sandstone) = match palette_idx {
        SAND | BEACH => (SAND, SANDSTONE_DEPTH),
        WATER => (WATER, 0),
        _ => (SOIL, 0),
    };
    let face = top - s;
    let filled = face - FILLER_DEPTH * s;
    if z >= face {
        palette_idx
    } else if z >= filled {
        filler
    } else if z >= filled - sandstone * s {
        SANDSTONE
    } else if z >= rock::<Pixelet>(cell, face_height(cell, top)) {
        STONE
    } else {
        UNKNOWN
    }
}

/// Cell `z` past the service area: the Earth's ground, bedrock under sea
/// level, air over it.
pub fn earth(z: i32) -> u32 {
    if z < SEA_LEVEL {
        UNKNOWN
    } else {
        VOID
    }
}
