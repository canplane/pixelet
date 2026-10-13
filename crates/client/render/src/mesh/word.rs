// pxl-render/mesh/word.rs

//! A box's texture taps, packed.

pub const NO_LAYER: u32 = 255;

/// One layer in all four of a box's texture taps.
pub fn taps_of(layer_idx: u8) -> u32 {
    layer_idx as u32 * 0x0101_0101
}
