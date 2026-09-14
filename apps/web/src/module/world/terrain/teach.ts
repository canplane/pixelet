// @pxl/web/module/world/terrain/teach.ts

/**
 * What the module is taught about the terrain's materials
 * (crates/pxl-core/src/bake/terrain.rs): the channels' classes, the bare
 * base, and each type's colour and layer from the palette's tables
 * (render/gpu/layer_table.ts). Read here once; the reference reading in
 * reference/terrain_layer.ts uses the same tables, which is what the tests
 * hold the module to.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { GROUND_COLOR_LOOKUP_FLAT, GROUND_LAYER_INDEX_LOOKUP_FLAT } from "@/render/gpu/layer_table.ts";
import { MaterialClass, NO_LAYER } from "@/render/mesh/voxel.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";

export const CH_TO_CLASS: Record<GroundChannelKey, MaterialClass> = {
  [GroundChannelKey.LAND]: MaterialClass.LAND,
  [GroundChannelKey.WATER]: MaterialClass.WATER,
  [GroundChannelKey.STRUCTURE]: MaterialClass.STRUCTURE,
  [GroundChannelKey.TRANSPORT]: MaterialClass.TRANSPORT,
};

/** The bare-ground base every cell starts from, before its channels. */
const BASE_TYPE = cube_types.land.BARE;
const BASE_FLAT = GROUND_COLOR_LOOKUP_FLAT[GroundChannelKey.LAND];
const BASE_LAYER = GROUND_LAYER_INDEX_LOOKUP_FLAT[GroundChannelKey.LAND];
export const BASE_R = BASE_FLAT?.has[BASE_TYPE] ? BASE_FLAT.rgb[BASE_TYPE * 3] : 0;
export const BASE_G = BASE_FLAT?.has[BASE_TYPE] ? BASE_FLAT.rgb[BASE_TYPE * 3 + 1] : 0;
export const BASE_B = BASE_FLAT?.has[BASE_TYPE] ? BASE_FLAT.rgb[BASE_TYPE * 3 + 2] : 0;
export const BASE_LAYER_IDX: u32 = BASE_LAYER?.has[BASE_TYPE] ? BASE_LAYER.index[BASE_TYPE] : NO_LAYER;

/** The channels in the order the module numbers them. */
const CHANNEL_INDEX: Record<GroundChannelKey, u32> = {
  [GroundChannelKey.LAND]: 0,
  [GroundChannelKey.WATER]: 1,
  [GroundChannelKey.STRUCTURE]: 2,
  [GroundChannelKey.TRANSPORT]: 3,
};

/** Hands the module the tables a cell's material is read from: the base,
 * each channel's class, and every type's colour and layer. Once, before
 * any page is told. */
export function teach_terrain(): void {
  if (CORE_PXL === null) return;
  CORE_PXL.pxl_terrain_base(BASE_R, BASE_G, BASE_B, BASE_LAYER_IDX, MaterialClass.LAND);
  for (const ch_key of GROUND_CH_KEYS) {
    const ch = CHANNEL_INDEX[ch_key];
    CORE_PXL.pxl_terrain_channel(ch, CH_TO_CLASS[ch_key]);
    const color = GROUND_COLOR_LOOKUP_FLAT[ch_key];
    const layer = GROUND_LAYER_INDEX_LOOKUP_FLAT[ch_key];
    for (let t = 0; t < 256; t++) {
      const has_color = color?.has[t] ? 1 : 0;
      const has_layer = layer?.has[t] ? 1 : 0;
      CORE_PXL.pxl_terrain_type(ch, t, color?.rgb[t * 3] ?? 0, color?.rgb[t * 3 + 1] ?? 0, color?.rgb[t * 3 + 2] ?? 0, has_color, layer?.index[t] ?? 0, has_layer);
    }
  }
}
