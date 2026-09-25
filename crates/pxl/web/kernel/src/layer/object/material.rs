// pxl-kernel/layer/object/material.rs

//! A placed cube's material — what the kernel is taught for its kind
//! (teach.rs).

use pxl_render::mesh::paint::palette::shader_meta::{ground_color_lookup, ground_layer_index_lookup, rgb255};
use pxl_render::mesh::paint::palette::tex_array::OBJECT_LAYER_IDX;
use pxl_render::mesh::volume::{MaterialClasses, VoxelMaterial, NO_LAYER};
use pxl_spec::cell::object::{placeable_cube_rgb, terrain_style_cube_source, PlaceableCube};
use xpute_core::status::bug::OrBug;
use xpute_core::status::errno::Errno;

/// A placed "solid" cube's own real color/layer_idx — either a flat color or,
/// for a terrain-styled pick, the exact same color/layer_idx
/// real baked terrain of that type gets, read straight from pxl-render's mesh/paint/palette/shader_meta.rs's
/// lookup tables. material_class stays OBJECT either way — the mesher's own
/// open-bottom/occlusion-exemption/run-merge behavior for OBJECT-classed runs
/// has nothing to do with which texture a cube samples, and reusing a terrain
/// class here would stop a solid object shielding whatever is beneath it from
/// occlusion culling, for a distinction layer-tap blending never needs: it only cares whether layer_idx is real.
pub fn resolve_solid_material(cube: PlaceableCube) -> VoxelMaterial {
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
        // The layer array is built by walking the blocks (crates/pxl/web/render/src/gpu/layers.rs),
        // so the index into it is that walk's.
        layer_idx: match cube {
            PlaceableCube::Block(block) => OBJECT_LAYER_IDX(block).unwrap_or(NO_LAYER),
            _ => NO_LAYER,
        },
    }
}
