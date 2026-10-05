// pxl-render/mesh/paint/object/material.rs

//! A placed cube's material, from the catalog and the palette.

use crate::mesh::paint::palette::shader_meta::rgb255;
use crate::mesh::paint::palette::tex_array::LAYER_IDX;
use crate::mesh::volume::{pack_material, MaterialClasses};
use pxl_spec::cell::object::{placeable_cube_rgb, PlaceableCube};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// Its material word and texture layer. A cube of a look takes its ground's
/// color and layer, but the class stays OBJECT: a terrain class would stop the
/// cube shielding what is beneath it from occlusion culling.
pub fn solid_material(cube: PlaceableCube) -> (u32, u8) {
    let rgb = rgb255(placeable_cube_rgb(cube).or_bug(Errno::ENOTRECOVERABLE));
    (pack_material(MaterialClasses::OBJECT, rgb), LAYER_IDX(cube) as u8)
}
