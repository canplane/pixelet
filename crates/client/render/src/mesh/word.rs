// pxl-render/mesh/word.rs

//! What a run's cell carries, packed: its material and class, its texture
//! layers, and the box inside it a summary shrinks a sparse cell to.

use xpute_kit::math::scalar::round;

pub const NO_LAYER: u32 = 255;

/// One layer in all four of a box's texture taps.
pub fn taps_of(layer_idx: u8) -> u32 {
    layer_idx as u32 * 0x0101_0101
}

pub const SUB_CELL_NONE: u8 = 0;
const SUB_CELL_SCALE: f64 = 256.0;
/// Read as exactly 1, not 255/256: a coarse box filling one axis while
/// shrinking another is ordinary, and 0 stays free for SUB_CELL_NONE.
const SUB_CELL_FULL: u8 = 255;

pub fn pack_sub_cell_fraction(fraction: f64) -> u32 {
    round(fraction * SUB_CELL_SCALE).clamp(0.0, SUB_CELL_SCALE - 1.0) as u32
}

pub fn unpack_sub_cell_fraction(packed: u8) -> f64 {
    packed as f64 / SUB_CELL_SCALE
}

pub fn unpack_sub_cell_size(packed: u8) -> f64 {
    if packed == SUB_CELL_FULL {
        1.0
    } else {
        packed as f64 / SUB_CELL_SCALE
    }
}

/// Which side of the world a cell is from: the base's ground, or a placed object.
pub struct Side;

impl Side {
    pub const BASE: u8 = 1;
    /// Exempt from hiding its neighbors' faces, and drawn with its bottom face.
    pub const OBJECT: u8 = 2;
}

/// A cell's material word, `0xSS_RRGGBB`: its side, then its color.
pub fn pack_material(side: u8, [r, g, b]: [u8; 3]) -> u32 {
    u32::from_be_bytes([side, r, g, b])
}

pub fn unpack_material(word: u32) -> (u8, [u8; 3]) {
    let [side, r, g, b] = word.to_be_bytes();
    (side, [r, g, b])
}
