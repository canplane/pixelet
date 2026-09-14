//! What a voxel's bytes mean — the part of render/layer/pipeline/voxel.ts the
//! mesher reads.

/// A run whose material is a placed object: exempt from hiding its
/// neighbours' faces, and drawn with its bottom face.
pub const MATERIAL_OBJECT: u8 = 5;
/// A coarse tier's cells, whatever they summarise.
pub const MATERIAL_LAND: u8 = 1;

/// No texture-array layer applies.
pub const NO_LAYER: u32 = 255;

/// A full-cell run: no inset.
pub const SUB_CELL_NONE: u8 = 0;
const SUB_CELL_SCALE: f64 = 256.0;
const SUB_CELL_FULL: u8 = 255;

/// A fraction of a cell packed into a byte, rounded to the nearest 1/256.
/// JavaScript's `Math.round` breaks ties upward and Rust's `round` away from
/// zero; for the non-negative fractions this packs, the two agree.
pub fn pack_sub_cell_fraction(fraction: f64) -> u32 {
    ((fraction * SUB_CELL_SCALE).round()).clamp(0.0, SUB_CELL_SCALE - 1.0) as u32
}

pub fn unpack_sub_cell_fraction(packed: u8) -> f64 {
    packed as f64 / SUB_CELL_SCALE
}

/// The largest packed size reads as exactly 1: a size of 1 is ordinary for a
/// coarse box that fills one axis.
pub fn unpack_sub_cell_size(packed: u8) -> f64 {
    if packed == SUB_CELL_FULL {
        1.0
    } else {
        packed as f64 / SUB_CELL_SCALE
    }
}
