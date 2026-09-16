// pxl_core/module/world/object/material.rs

//! A placed cube's material — what the kernel is taught for its kind
//! (teach.rs).

use pxl_shared::spec::cell::{placeable_cube_id, placeable_cube_rgb, terrain_style_cube_source, PlaceableCube};

use crate::render::gpu::layer_table::{ground_color_lookup, ground_layer_index_lookup, rgb255, OBJECT_LAYER_IDX};
use crate::render::mesh::voxel::{MaterialClasses, VoxelMaterial, NO_LAYER};

/// A placed "solid" cube's own real color/layer_idx — either a flat color or,
/// for a terrain-styled pick (2026-09-06), the exact same color/layer_idx
/// real baked terrain of that type gets, read straight from layer_table.rs's
/// lookup tables. material_class stays OBJECT either way — the mesher's own
/// open-bottom/occlusion-exemption/run-merge behavior for OBJECT-classed runs
/// has nothing to do with which texture a cube samples, and reusing a terrain
/// class here would trade away real, already-fixed correctness (a solid
/// object no longer shields whatever's beneath it from occlusion culling, the
/// mesher's own 2026-08-16 bug) for a distinction layer-tap blending never
/// needs: it only cares whether layer_idx is real.
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
    let [r, g, b] = rgb255(placeable_cube_rgb(cube).expect("a flat colour carries a swatch"));
    VoxelMaterial {
        material_class: MaterialClasses::OBJECT,
        r: r as u32,
        g: g as u32,
        b: b as u32,
        // The layer array is built by walking cube_ids (render/gpu/layers.rs),
        // so the index into it is that walk's, which goes by the id.
        layer_idx: OBJECT_LAYER_IDX(&placeable_cube_id(cube)).unwrap_or(NO_LAYER),
    }
}
