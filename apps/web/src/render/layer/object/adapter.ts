// @pxl/web/render/layer/object/adapter.ts

/**
 * Sparse placed objects -> shared temporary 64x64 chunk. The paint is the
 * module's (crates/pxl-core/src/object.rs's Painter) over the store's
 * records, which are the module's too; what this file does is teach it
 * what each state paints as, once, and hand it the drawn build's edits.
 */

import type { u32 } from "@xpute/core/kit/word.ts";

import {
  cube_hex,
  cube_types,
  decor_ids,
  decor_num_id,
  is_terrain_style_cube,
  placeable_cube_ids,
  placeable_cube_num_id,
  type PlaceableCubeId,
  terrain_style_cube_source,
} from "@pxl/shared/spec/cell/mod.ts";
import { FROM_L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { column_render_edits, object_column_of, object_column_revision } from "@/module/world/object/store.ts";
import { GROUND_COLOR_LOOKUP_FLAT, GROUND_LAYER_INDEX_LOOKUP_FLAT, hex_to_rgb255, OBJECT_LAYER_IDX } from "../terrain/shader_meta.ts";

import { type ChunkVolume, note_paint } from "../pipeline/volume.ts";
import { MaterialClass, NO_LAYER, pack_material, type VoxelMaterial } from "../pipeline/voxel.ts";
import type { ChunkKey, WorldLayer, WorldLayerRevision } from "../pipeline/contract.ts";
import { CORE_PXL } from "../pipeline/core_pool.ts";

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

/** What the module's paint takes a state for: object.rs's Kind. */
const KIND_SOLID: u32 = 1;
const KIND_AIR: u32 = 2;
const KIND_DECOR: u32 = 3;

/** Teaches the module every state the catalog can put in a leaf: each
 * placeable cube's material, AIR, and the decor kinds, which paint
 * nothing. Once, at import, as the terrain adapter teaches its tables. */
function teach_module(): void {
  const p = CORE_PXL;
  if (p === null) return;
  for (const id of placeable_cube_ids) {
    const m = resolve_solid_material(id);
    p.pxl_object_kind(placeable_cube_num_id(id), KIND_SOLID, pack_material(m), m.layer_idx);
  }
  p.pxl_object_kind(cube_types.common.AIR, KIND_AIR, 0, NO_LAYER);
  for (const id of decor_ids) p.pxl_object_kind(decor_num_id(id), KIND_DECOR, 0, NO_LAYER);
}

teach_module();

export class ObjectWorldLayer implements WorldLayer {
  readonly name = "objects";

  revision(key: ChunkKey): WorldLayerRevision {
    return [object_column_revision(object_column_of(key.page_idx, key.page_off))];
  }

  paint(key: ChunkKey, target: ChunkVolume, opts?: { for_raycast?: boolean }): boolean {
    const p = CORE_PXL;
    if (p === null) throw new Error("render/object: the object layer paints through the module");
    const base = object_column_of(key.page_idx, key.page_off);
    // A raycast paints the real set; only the drawn build sees the eraser
    // preview's edits (store.ts's set_virtual_hide).
    const e = column_render_edits(base);
    const render = !opts?.for_raycast;
    const made = render ? p.pxl_object_paint(target.id, base, e.hide, e.hide_band, e.hide_pos, e.carve, e.lx, e.ly, e.lz, e.size) : p.pxl_object_paint(target.id, base, 0, 0, 0, 0, 0, 0, 0, 0);
    note_paint(target, (made & 2) === 0 ? 1 : 0);
    return (made & 1) !== 0;
  }
}
