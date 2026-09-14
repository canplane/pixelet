// @pxl/web/module/world/object/teach.ts

/**
 * What the module is taught about the objects' states
 * (crates/pxl/core/src/world/object.rs): every placeable cube's material,
 * AIR, and the decor kinds. Once, before any page is told.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { cube_types, decor_ids, decor_num_id, placeable_cube_ids, placeable_cube_num_id } from "@pxl/shared/spec/cell/mod.ts";

import { resolve_solid_material } from "@/module/world/object/material.ts";
import { NO_LAYER, pack_material } from "@/render/mesh/voxel.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";

/** What the module's paint takes a state for: object.rs's Kind. */
const KIND_SOLID: u32 = 1;
const KIND_AIR: u32 = 2;
const KIND_DECOR: u32 = 3;

/** Teaches the module every state the catalog can put in a leaf: each
 * placeable cube's material, AIR, and the decor kinds, which paint
 * nothing. Once, at import, as the terrain adapter teaches its tables. */
export function teach_objects(): void {
  const p = CORE_PXL;
  if (p === null) return;
  for (const id of placeable_cube_ids) {
    const m = resolve_solid_material(id);
    p.pxl_object_kind(placeable_cube_num_id(id), KIND_SOLID, pack_material(m), m.layer_idx);
  }
  p.pxl_object_kind(cube_types.common.AIR, KIND_AIR, 0, NO_LAYER);
  for (const id of decor_ids) p.pxl_object_kind(decor_num_id(id), KIND_DECOR, 0, NO_LAYER);
}
