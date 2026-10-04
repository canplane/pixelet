// pxl-render/mesh/paint/object/material.rs

//! A placed cube's material, from the catalog and the palette.

use crate::mesh::paint::palette::shader_meta::rgb255;
use crate::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;
use crate::mesh::paint::terrain::adapter::MATERIALS;
use crate::mesh::volume::{pack_material, MaterialClasses, NO_LAYER};
use pxl_spec::bake::format::ch_index;
use pxl_spec::cell::object::{placeable_cube_rgb, terrain_style_cube_source, PlaceableCube};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// A terrain-styled pick takes the exact color and layer its terrain type gets.
/// The class stays OBJECT: a terrain class would stop the cube shielding what
/// is beneath it from occlusion culling.
/// Its material word and texture layer.
pub fn solid_material(cube: PlaceableCube) -> (u32, u8) {
    if let Some(source) = terrain_style_cube_source(cube) {
        let (c, t) = (&MATERIALS.channels[ch_index(source.channel)], source.terrain_num_id as usize);
        let (rgb, layer) = if c.has[t] { (c.rgb[t], c.layer[t]) } else { ([0; 3], NO_LAYER as u8) };
        return (pack_material(MaterialClasses::OBJECT, rgb), layer);
    }
    let rgb = rgb255(placeable_cube_rgb(cube).or_bug(Errno::ENOTRECOVERABLE));
    // The layer array is built by walking the blocks, so the index is that walk's.
    let layer = match cube {
        PlaceableCube::Block(block) => OBJECT_LAYER_IDX(block).unwrap_or(NO_LAYER),
        _ => NO_LAYER,
    };
    (pack_material(MaterialClasses::OBJECT, rgb), layer as u8)
}
