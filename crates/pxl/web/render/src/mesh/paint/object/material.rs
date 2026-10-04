// pxl-render/mesh/paint/object/material.rs

//! A placed cube's material, from the catalog and the palette.

use crate::mesh::paint::palette::shader_meta::{ground_color_lookup, ground_layer_index_lookup, rgb255};
use crate::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;
use crate::mesh::volume::{MaterialClasses, VoxelMaterial, NO_LAYER};
use pxl_spec::cell::object::{placeable_cube_rgb, terrain_style_cube_source, PlaceableCube};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// A terrain-styled pick takes the exact color and layer its terrain type gets.
/// The class stays OBJECT: a terrain class would stop the cube shielding what
/// is beneath it from occlusion culling.
pub fn solid_material(cube: PlaceableCube) -> VoxelMaterial {
    if let Some(source) = terrain_style_cube_source(cube) {
        let t = source.terrain_num_id as usize;
        let color = ground_color_lookup(source.channel).filter(|c| c.has[t] != 0);
        let layer = ground_layer_index_lookup(source.channel).filter(|l| l.has[t] != 0);
        return VoxelMaterial {
            material_class: MaterialClasses::OBJECT,
            r: color.map_or(0, |c| c.rgb[t * 3] as u32),
            g: color.map_or(0, |c| c.rgb[t * 3 + 1] as u32),
            b: color.map_or(0, |c| c.rgb[t * 3 + 2] as u32),
            layer_idx: layer.map_or(NO_LAYER, |l| l.index[t] as u32),
        };
    }
    let [r, g, b] = rgb255(placeable_cube_rgb(cube).or_bug(Errno::ENOTRECOVERABLE));
    VoxelMaterial {
        material_class: MaterialClasses::OBJECT,
        r: r as u32,
        g: g as u32,
        b: b as u32,
        // The layer array is built by walking the blocks, so the index is that walk's.
        layer_idx: match cube {
            PlaceableCube::Block(block) => OBJECT_LAYER_IDX(block).unwrap_or(NO_LAYER),
            _ => NO_LAYER,
        },
    }
}
