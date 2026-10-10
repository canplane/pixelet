// pxl-render/mesh/paint/palette/tex_array.rs

use pxl_spec::cell::cube_types;

use pxl_spec::cell::object::block_cubes;

use crate::mesh::paint::palette::shader_meta::GROUND_LAYER_META;

/// Object colors, numbered right after every ground layer. Only the 15 flat
/// colors: terrain-styled cubes reuse their ground layer rather than get their own.
#[allow(non_snake_case)]
pub fn OBJECT_LAYER_IDX(block: cube_types::Block) -> Option<u32> {
    block_cubes().iter().position(|b| *b == block).map(|i| (GROUND_LAYER_META.len() + i) as u32)
}
