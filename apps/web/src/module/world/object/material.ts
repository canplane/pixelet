// @pxl/web/module/world/object/material.ts

/**
 * A placed cube's material — what the module is taught for its kind
 * (teach.ts) and what the reference paint writes (reference/object_layer.ts).
 */

import { cube_hex, is_terrain_style_cube, type PlaceableCubeId, terrain_style_cube_source } from "@pxl/shared/spec/cell/mod.ts";

import { GROUND_COLOR_LOOKUP_FLAT, GROUND_LAYER_INDEX_LOOKUP_FLAT, hex_to_rgb255, OBJECT_LAYER_IDX } from "@/render/gpu/layer_table.ts";
import { MaterialClass, NO_LAYER, type VoxelMaterial } from "@/render/mesh/voxel.ts";

/**
 * A placed "solid" cube's own real color/layer_idx — either a flat color
 * (this file's own long-standing path) or, for a terrain-styled pick
 * (cell/mod.ts's own TERRAIN_STYLE_SOURCE, 2026-09-06), the exact same
 * color/layer_idx real baked terrain of that type gets, read straight from
 * layer_table.ts's lookup tables rather than this file's own flat-color
 * ones. material_class stays MaterialClass.OBJECT either way — the mesher's
 * own open-bottom/occlusion-exemption/run-merge behavior for OBJECT-classed
 * runs (mesher.rs) has nothing to do with which
 * texture a cube samples, and reusing a terrain MaterialClass here would
 * trade away real, already-fixed correctness (a solid object no longer
 * shields whatever's beneath it from occlusion culling, the mesher's
 * own 2026-08-16 bug) for a distinction this function's own two branches
 * already don't need it for: layer-tap blending itself only cares whether
 * layer_idx is real, never material_class.
 */
export function resolve_solid_material(color_id: PlaceableCubeId): VoxelMaterial {
  if (is_terrain_style_cube(color_id)) {
    const { channel, terrain_num_id } = terrain_style_cube_source(color_id);
    const color_flat = GROUND_COLOR_LOOKUP_FLAT[channel];
    const layer_flat = GROUND_LAYER_INDEX_LOOKUP_FLAT[channel];
    const off = terrain_num_id * 3;
    const has_color = color_flat?.has[terrain_num_id];
    return {
      material_class: MaterialClass.OBJECT,
      r: has_color ? color_flat.rgb[off] : 0,
      g: has_color ? color_flat.rgb[off + 1] : 0,
      b: has_color ? color_flat.rgb[off + 2] : 0,
      layer_idx: layer_flat?.has[terrain_num_id] ? layer_flat.index[terrain_num_id] : NO_LAYER,
    };
  }
  const [r, g, b] = hex_to_rgb255(cube_hex(color_id));
  const layer_idx = OBJECT_LAYER_IDX[color_id] ?? NO_LAYER;
  return { material_class: MaterialClass.OBJECT, r, g, b, layer_idx };
}
