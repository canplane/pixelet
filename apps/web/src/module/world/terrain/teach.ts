// @pxl/web/module/world/terrain/teach.ts

/**
 * What the module is taught about the terrain's materials
 * (crates/pxl/core/src/bake/terrain.rs): the channels' classes, the bare
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
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { CORE_BUILD, packet } from "@/wasm/core_build.ts";

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

const NO_RGB = new Uint8Array(256 * 3);
const NO_TYPES = new Uint8Array(256);

/** Hands the module the tables a cell's material is read from, whole, in
 * one command: the base, and each channel's class and every type's colour
 * and layer, in GROUND_CH_KEYS' order. Once, before any page is told. */
export function teach_terrain(): void {
  CORE_BUILD.send(
    0,
    Cmd.TERRAIN_UPDATE,
    packet((p) =>
      p.branch((base) => base.u32(BASE_R).u32(BASE_G).u32(BASE_B).u32(BASE_LAYER_IDX).u32(MaterialClass.LAND))
        .branch((channels) => {
          for (const ch_key of GROUND_CH_KEYS) {
            const color = GROUND_COLOR_LOOKUP_FLAT[ch_key];
            const layer = GROUND_LAYER_INDEX_LOOKUP_FLAT[ch_key];
            channels.branch((c) => c.u32(CH_TO_CLASS[ch_key]).u8_array(color?.rgb ?? NO_RGB).u8_array(color?.has ?? NO_TYPES).u8_array(layer?.index ?? NO_TYPES).u8_array(layer?.has ?? NO_TYPES));
          }
        })
    ),
  );
}
