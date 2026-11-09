// pxl-content/strata.rs

//! wgsl-twin: the ground's strata by depth under a column's face, after Minecraft Beta's
//! `replaceBlocksForBiome`: the face's look on top, a filler of soil, sand or water,
//! sandstone under sand, and stone under all, down to the bedrock. It also says how high a
//! cube may reach above the face, and what the Earth's ground is past the service area.
//!
//! It is where the world ends: the one place for every limit an edit and a column meet. The
//! world is open from `BEDROCK_DEPTH` under the ground or sea level, whichever is lower, to
//! `HEADROOM` over the ground or sea level, whichever is higher, in leaf cells (`bedrock`,
//! `ceiling`). Under the floor is bedrock, the unknown of which the world holds no more
//! (`spec/spec/asset/cube.json` `unknown`), and the service area is walled with it from
//! inside: a leaf column with ground beside one past the area is bedrock up to its own face, a
//! cell of the leaves like any other, so the wall's outer face stands on the area's edge at
//! every zoom (pxl-spec area.rs `is_wall`). A walker and an edit keep off the wall's columns
//! by the rule the wall is laid by (pxl-core domain/query/limit.rs `within`, `editable`).

use pxl_engine::ground::rock;
use pxl_spec::ground::{face_height, leaf_cells, UNKNOWN, VOID};
use pxl_spec::space::coord::world::SEA_LEVEL;
use pxl_spec::space::coord::zoom::{Zoom, LEAF_CELL_ZOOM};

use crate::param::strata::{BEDROCK_DEPTH, FILLER_DEPTH, HEADROOM, SANDSTONE_DEPTH};
use crate::Pixelet;
use pxl_spec::asset::cube::ground;

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
