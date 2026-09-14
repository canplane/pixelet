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
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";

/** What the module's paint takes a state for: object.rs's Kind. */
const KIND_SOLID: u32 = 1;
const KIND_AIR: u32 = 2;
const KIND_DECOR: u32 = 3;

/** Teaches the module every state the catalog can put in a leaf: each
 * placeable cube's material, AIR, and the decor kinds, which paint
 * nothing. Once, at import, as the terrain adapter teaches its tables. */
export function teach_objects(): void {
  const teach = (state: u32, kind: u32, material: u32, layer: u32) => CORE_BUILD.send(0, Cmd.STATE_UPDATE, packet((p) => p.u32(state).u32(kind).u32(material >>> 0).u32(layer)));
  for (const id of placeable_cube_ids) {
    const m = resolve_solid_material(id);
    teach(placeable_cube_num_id(id), KIND_SOLID, pack_material(m), m.layer_idx);
  }
  teach(cube_types.common.AIR, KIND_AIR, 0, NO_LAYER);
  for (const id of decor_ids) teach(decor_num_id(id), KIND_DECOR, 0, NO_LAYER);
}
