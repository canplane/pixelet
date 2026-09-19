// pxl-render/mesh/paint/palette/tex_array.rs

use pxl_spec::cell::cube_types;

use pxl_spec::cell::object::block_cubes;

use crate::mesh::paint::palette::shader_meta::GROUND_LAYER_META;

/// Object colors, numbered starting right after every ground layer — layers
/// builds the array in this order, and object/material.rs reads this to
/// resolve a placed cube's layer_idx the same way the terrain reads
/// GROUND_LAYER_INDEX_LOOKUP_FLAT. A numbering, not a texture: what a placed
/// cube's layer is must be answerable where no canvas exists.
///
/// The blocks are only ever the original 15 flat colors — the terrain-styled
/// catalog is a separate numbering, deliberately never mixed into this one:
/// those cubes are meant to look identical to an existing ground layer, not
/// get a new one of their own.
#[allow(non_snake_case)]
pub fn OBJECT_LAYER_IDX(block: cube_types::Block) -> Option<u32> {
    block_cubes().iter().position(|b| *b == block).map(|i| (GROUND_LAYER_META.len() + i) as u32)
}
