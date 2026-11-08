// pxl-content/strata.rs

//! gpu-portable: the ground's strata by depth under a column's face, after Minecraft Beta's
//! `replaceBlocksForBiome`: the face's look on top, a filler of soil, sand or water,
//! sandstone under sand, and stone under all, down to the bedrock. It also says how high a
//! cube may reach above the face, and what the Earth's ground is past the service area.

use pxl_engine::ground::{face_height, leaf_cells, rock, UNKNOWN, VOID};
use pxl_engine::{Zoom, LEAF_CELL_ZOOM, SEA_LEVEL};

use crate::cube::ground;
use crate::param::strata::{BEDROCK_DEPTH, FILLER_DEPTH, HEADROOM, SANDSTONE_DEPTH};
use crate::Pixelet;

// The palette entries the strata name, read into a word.
const SAND: u32 = ground::sand as u32;
const BEACH: u32 = ground::beach as u32;
const WATER: u32 = ground::water as u32;
const SOIL: u32 = ground::soil as u32;
const SANDSTONE: u32 = ground::sandstone as u32;
const STONE: u32 = ground::stone as u32;

/// The top of the bedrock under a face `face` leaf cells high, in leaf cells: the lowest a
/// cube may reach there, for every walk, edit and column.
pub fn bedrock(face: i32) -> i32 {
    face.min(SEA_LEVEL) - BEDROCK_DEPTH
}

pub fn ceiling(face: i32) -> i32 {
    face.max(SEA_LEVEL) + HEADROOM
}

/// What the ground is in cell `z` of a column at `cell_zoom` whose palette index is
/// `palette_idx` and whose top is `top` cells: by depth under its top, and void above it. A
/// column coarser than the leaves' summarizes the faces under it, so its ground is its face
/// all through; the wall (`unknown`) is itself all through.
pub fn by_depth(cell_zoom: Zoom, palette_idx: u32, top: i32, z: i32) -> u32 {
    if z >= top {
        return VOID;
    }
    if cell_zoom < LEAF_CELL_ZOOM || palette_idx == UNKNOWN {
        return palette_idx;
    }
    let s = leaf_cells(cell_zoom);
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
    } else if z >= rock::<Pixelet>(cell_zoom, face_height(cell_zoom, top)) {
        STONE
    } else {
        UNKNOWN
    }
}

/// Cell `z` past the service area: the Earth's ground, which is bedrock under sea level and
/// air above it.
pub fn earth(z: i32) -> u32 {
    if z < SEA_LEVEL {
        UNKNOWN
    } else {
        VOID
    }
}
