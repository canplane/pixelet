// @pxl/web/render/layer/object/adapter.ts

/** Sparse placed objects -> shared temporary 64x64 chunk. */

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { IndexedMap, U32_KEYS } from "@xpute/core/ds/map.ts";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constant.ts";
import { cube_hex, is_terrain_style_cube, object_value_is_air, object_value_is_decor, placeable_cube_id_from_num, type PlaceableCubeId, terrain_style_cube_source } from "@pxl/shared/spec/cell/mod.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { column_has_entries, each_column_entry, object_column_of, object_column_revision } from "@/module/world/object/store.ts";
import { OBJECT_LAYER_IDX } from "../../tex_array.ts";
import { GROUND_COLOR_LOOKUP_FLAT, GROUND_LAYER_INDEX_LOOKUP_FLAT, hex_to_rgb255 } from "../terrain/shader_meta.ts";

import { append_chunk_sub_voxel, type ChunkVolume, MaterialClass, NO_LAYER, pack_sub_cell, pack_sub_cell_fraction, set_chunk_voxel, SUB_CELL_NONE, type VoxelMaterial } from "../pipeline/volume.ts";
import type { ChunkKey, WorldLayer, WorldLayerRevision } from "../pipeline/contract.ts";
import { CORE_ALLOC } from "../pipeline/core_pool.ts";

export function chunk_origin_of(page_idx: IVec, page_off: IVec): IVec {
  return ivec.add(FROM_L3_IDX(page_idx), page_off);
}

/**
 * A placed "solid" cube's own real color/layer_idx — either a flat color
 * (this file's own long-standing path) or, for a terrain-styled pick
 * (cell/mod.ts's own TERRAIN_STYLE_SOURCE, 2026-09-06), the exact same
 * color/layer_idx real baked terrain of that type gets, read straight from
 * shader_meta.ts's own lookup tables rather than this file's own flat-color
 * ones. material_class stays MaterialClass.OBJECT either way — chunk_mesher.ts's
 * own open-bottom/occlusion-exemption/run-merge behavior for OBJECT-classed
 * runs (see that file's own doc comments) has nothing to do with which
 * texture a cube samples, and reusing a terrain MaterialClass here would
 * trade away real, already-fixed correctness (a solid object no longer
 * shields whatever's beneath it from occlusion culling, chunk_mesher.ts's
 * own 2026-08-16 bug) for a distinction this function's own two branches
 * already don't need it for: layer-tap blending itself only cares whether
 * layer_idx is real, never material_class.
 */
/**
 * The voxels this paint has claimed, so a second sub-cell object landing on
 * a claimed one is filed as overflow rather than overwriting it. Keyed by
 * the packed cell address — chunk-local x and y in six bits each and the
 * absolute cell Z, biased to stay positive, above — and cleared per paint.
 */
const claimed = new IndexedMap(U32_KEYS, 256, CORE_ALLOC);
const Z_BIAS: u32 = 1 << 19;
const voxel_address = (cx: u32, cy: u32, z: i32): u32 => (((z + Z_BIAS) << 12) | (cy << 6) | cx) >>> 0;

function resolve_solid_material(color_id: PlaceableCubeId): VoxelMaterial {
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

export class ObjectWorldLayer implements WorldLayer {
  readonly name = "objects";

  revision(key: ChunkKey): WorldLayerRevision {
    return [object_column_revision(object_column_of(key.page_idx, key.page_off))];
  }

  /** Placed objects carve, stack and sit off the cell grid, none of which a
   * plain column run expresses; a column that holds any keeps the scan path
   * (WorldLayer.runs). */
  present(key: ChunkKey): boolean {
    return column_has_entries(object_column_of(key.page_idx, key.page_off));
  }

  paint(key: ChunkKey, target: ChunkVolume, opts?: { for_raycast?: boolean }): boolean {
    claimed.clear();
    let any = false;

    // A raycast paints the real set; only the drawn build sees the eraser
    // preview's edits (store.ts's set_virtual_hide).
    each_column_entry(object_column_of(key.page_idx, key.page_off), (obj) => {
      // Decor (a user-placed tree, say) never paints a voxel of its own —
      // it renders as decor_layer.ts's own separate instanced GLB geometry,
      // not a chunk_mesher.ts box, and has no colour to paint with anyway.
      if (object_value_is_decor(obj.state)) return;

      const local_x = obj.lx;
      const local_y = obj.ly;
      const base_z_real = obj.lz;
      const is_sub_cell = obj.size < U_PER_CELL;
      const cx0 = is_sub_cell ? Math.max(0, Math.floor(local_x / U_PER_CELL)) : Math.max(0, Math.round(local_x / U_PER_CELL));
      const cy0 = is_sub_cell ? Math.max(0, Math.floor(local_y / U_PER_CELL)) : Math.max(0, Math.round(local_y / U_PER_CELL));
      const z = is_sub_cell ? Math.floor(base_z_real / U_PER_CELL) : Math.round(base_z_real / U_PER_CELL);
      if (cx0 >= CHUNK_CELL_SPAN || cy0 >= CHUNK_CELL_SPAN) return;

      const sz_cells = Math.max(1, Math.round(obj.size / U_PER_CELL));
      const cx1 = Math.min(CHUNK_CELL_SPAN, cx0 + sz_cells);
      const cy1 = Math.min(CHUNK_CELL_SPAN, cy0 + sz_cells);
      const is_air = object_value_is_air(obj.state);
      const material: VoxelMaterial = is_air ? { material_class: MaterialClass.AIR, r: 0, g: 0, b: 0, layer_idx: NO_LAYER } : resolve_solid_material(placeable_cube_id_from_num(obj.state));

      const sub_sz = is_sub_cell ? pack_sub_cell_fraction(obj.size / U_PER_CELL) : SUB_CELL_NONE;
      const sub_ox = is_sub_cell ? pack_sub_cell_fraction((local_x - cx0 * U_PER_CELL) / U_PER_CELL) : 0;
      const sub_oy = is_sub_cell ? pack_sub_cell_fraction((local_y - cy0 * U_PER_CELL) / U_PER_CELL) : 0;
      const sub_oz = is_sub_cell ? pack_sub_cell_fraction((base_z_real - z * U_PER_CELL) / U_PER_CELL) : 0;

      for (let cy: u32 = cy0; cy < cy1; cy++) {
        for (let cx: u32 = cx0; cx < cx1; cx++) {
          const address = voxel_address(cx, cy, z);
          any = true;
          if (claimed.find(address) >= 0 && !is_air) {
            append_chunk_sub_voxel(target, cx, cy, z, material, pack_sub_cell(sub_ox, sub_oy, sub_oz, sub_sz));
          } else {
            claimed.insert(address);
            set_chunk_voxel(target, cx, cy, z, material, is_sub_cell ? [sub_ox, sub_oy, sub_oz, sub_sz] : undefined);
          }
        }
      }
    }, !opts?.for_raycast);
    return any;
  }
}
