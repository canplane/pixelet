// @pxl/web/render/layer/objects/adapter.ts

/** Sparse placed objects -> shared temporary 64x64 chunk. */

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { object_color_hex } from "@pxl/shared/spec/cell/objects.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { fvec, type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { get_object_entries_in_l2_column, get_render_object_entries_in_l2_column, l2_column_key_of, type ObjectLayerEntry } from "@/modules/world/objects/store.ts";
import { OBJECT_LAYER_IDX } from "../../tex_array.ts";
import { hex_to_rgb255 } from "../terrain/shader_meta.ts";

import { append_chunk_sub_voxel, type ChunkVolume, MaterialClass, NO_LAYER, pack_sub_cell_fraction, set_chunk_voxel, SUB_CELL_NONE, type VoxelMaterial } from "../pipeline/volume.ts";
import type { ChunkKey, WorldLayer, WorldLayerRevision } from "../pipeline/contract.ts";

export function chunk_origin_of(page_idx: IVec, page_off: IVec): IVec {
  return ivec.add(FROM_L3_IDX(page_idx), page_off);
}

export function object_local_xyz(obj: ObjectLayerEntry, chunk_origin: IVec): [f64, f64, f64] {
  const local = ivec.sub(obj.i, chunk_origin);
  return [
    ivec.to_real(ivec.X(local)) + fvec.to_real(fvec.X(obj.f)),
    ivec.to_real(ivec.Y(local)) + fvec.to_real(fvec.Y(obj.f)),
    ivec.to_real(ivec.Z(local)) + fvec.to_real(fvec.Z(obj.f)),
  ];
}

function objects_in_chunk(key: ChunkKey): readonly ObjectLayerEntry[] {
  return get_object_entries_in_l2_column(l2_column_key_of(key.page_idx, key.page_off));
}

/** Same as objects_in_chunk, with set_virtual_hide's own render-only
 * exclusion applied — never used for revision() or a raycast paint, both of
 * which must see the real, unfiltered set (see set_virtual_hide's own doc
 * comment for why). */
function render_objects_in_chunk(key: ChunkKey): readonly ObjectLayerEntry[] {
  return get_render_object_entries_in_l2_column(l2_column_key_of(key.page_idx, key.page_off));
}

export class ObjectWorldLayer implements WorldLayer {
  readonly name = "objects";

  revision(key: ChunkKey): WorldLayerRevision {
    return [objects_in_chunk(key)];
  }

  paint(key: ChunkKey, target: ChunkVolume, opts?: { for_raycast?: boolean }): boolean {
    const objects = opts?.for_raycast ? objects_in_chunk(key) : render_objects_in_chunk(key);
    if (objects.length === 0) return false;

    const chunk_origin = chunk_origin_of(key.page_idx, key.page_off);
    const claimed = new Set<string>();
    let any = false;

    for (const obj of objects) {
      // Decor (a user-placed tree, say) never paints a voxel of its own —
      // it renders as decor_layer.tsx's own separate instanced GLB geometry,
      // not a chunk_mesher.ts box, and has no color_id to paint with anyway.
      if (obj.kind === "decor") continue;

      const [local_x, local_y, base_z_real] = object_local_xyz(obj, chunk_origin);
      const is_sub_cell = obj.size < U_PER_CELL;
      const cx0 = is_sub_cell ? Math.max(0, Math.floor(local_x / U_PER_CELL)) : Math.max(0, Math.round(local_x / U_PER_CELL));
      const cy0 = is_sub_cell ? Math.max(0, Math.floor(local_y / U_PER_CELL)) : Math.max(0, Math.round(local_y / U_PER_CELL));
      const z = is_sub_cell ? Math.floor(base_z_real / U_PER_CELL) : Math.round(base_z_real / U_PER_CELL);
      if (cx0 >= CHUNK_CELL_SPAN || cy0 >= CHUNK_CELL_SPAN) continue;

      const sz_cells = Math.max(1, Math.round(obj.size / U_PER_CELL));
      const cx1 = Math.min(CHUNK_CELL_SPAN, cx0 + sz_cells);
      const cy1 = Math.min(CHUNK_CELL_SPAN, cy0 + sz_cells);
      const material: VoxelMaterial = obj.kind === "air" ? { material_class: MaterialClass.AIR, r: 0, g: 0, b: 0, layer_idx: NO_LAYER } : (() => {
        const [r, g, b] = hex_to_rgb255(object_color_hex(obj.color_id));
        const layer_idx = OBJECT_LAYER_IDX[obj.color_id] ?? NO_LAYER;
        return { material_class: MaterialClass.OBJECT, r, g, b, layer_idx };
      })();

      const sub_sz = is_sub_cell ? pack_sub_cell_fraction(obj.size / U_PER_CELL) : SUB_CELL_NONE;
      const sub_ox = is_sub_cell ? pack_sub_cell_fraction((local_x - cx0 * U_PER_CELL) / U_PER_CELL) : 0;
      const sub_oy = is_sub_cell ? pack_sub_cell_fraction((local_y - cy0 * U_PER_CELL) / U_PER_CELL) : 0;
      const sub_oz = is_sub_cell ? pack_sub_cell_fraction((base_z_real - z * U_PER_CELL) / U_PER_CELL) : 0;

      for (let cy: u32 = cy0; cy < cy1; cy++) {
        for (let cx: u32 = cx0; cx < cx1; cx++) {
          const address = `${cx},${cy},${z}`;
          any = true;
          if (claimed.has(address) && obj.kind === "solid") {
            append_chunk_sub_voxel(target, { cx, cy, z, ...material, sub_sz, sub_ox, sub_oy, sub_oz });
          } else {
            claimed.add(address);
            set_chunk_voxel(target, cx, cy, z, material, is_sub_cell ? [sub_ox, sub_oy, sub_oz, sub_sz] : undefined);
          }
        }
      }
    }
    return any;
  }
}
