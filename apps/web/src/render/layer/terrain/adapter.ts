// @pxl/web/render/layer/terrain/adapter.ts

/** L3 heightfield -> temporary dense signed-Z render bands. */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { I16Array } from "@xpute/core/kit/array.ts";

import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { useTileStore } from "@/module/world/terrain/store.ts";
import { slice_chunk } from "./chunk.ts";
import { GROUND_COLOR_LOOKUP_FLAT, GROUND_LAYER_INDEX_LOOKUP_FLAT } from "./shader_meta.ts";

import { type ChunkVolume, fill_chunk_column, L2_BAND_CELL_SPAN, MaterialClass, NO_LAYER, type VoxelMaterial } from "../pipeline/volume.ts";
import type { ColumnRunWriter } from "../pipeline/voxel_scan.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import type { ChunkKey, WorldLayer, WorldLayerRevision } from "../pipeline/contract.ts";

const CHUNK_CELL_SPAN2 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;
const TERRAIN_Z_MARGIN = 4;

const CH_TO_CLASS: Record<GroundChannelKey, MaterialClass> = {
  [GroundChannelKey.LAND]: MaterialClass.LAND,
  [GroundChannelKey.WATER]: MaterialClass.WATER,
  [GroundChannelKey.STRUCTURE]: MaterialClass.STRUCTURE,
  [GroundChannelKey.TRANSPORT]: MaterialClass.TRANSPORT,
};

type RenderGround = ReturnType<typeof slice_chunk>["ground"];

function ground_material_at(ground: RenderGround, idx: u32, show_structures: boolean): VoxelMaterial {
  const base_type = cube_types.land.BARE;
  const base_flat = GROUND_COLOR_LOOKUP_FLAT[GroundChannelKey.LAND];
  let r = base_flat?.has[base_type] ? base_flat.rgb[base_type * 3] : 0;
  let g = base_flat?.has[base_type] ? base_flat.rgb[base_type * 3 + 1] : 0;
  let b = base_flat?.has[base_type] ? base_flat.rgb[base_type * 3 + 2] : 0;
  let material_class = MaterialClass.LAND;
  const base_layer_idx = GROUND_LAYER_INDEX_LOOKUP_FLAT[GroundChannelKey.LAND];
  let layer_idx: u32 = base_layer_idx?.has[base_type] ? base_layer_idx.index[base_type] : NO_LAYER;

  for (const ch_key of GROUND_CH_KEYS) {
    if (ch_key === GroundChannelKey.STRUCTURE && !show_structures) continue;
    const type_id = ground[ch_key][idx];
    if (type_id === cube_types.common.VOID) continue;
    const flat = GROUND_COLOR_LOOKUP_FLAT[ch_key];
    if (!flat?.has[type_id]) continue;

    const off = type_id * 3;
    r = flat.rgb[off];
    g = flat.rgb[off + 1];
    b = flat.rgb[off + 2];
    material_class = CH_TO_CLASS[ch_key];
    const layer_flat = GROUND_LAYER_INDEX_LOOKUP_FLAT[ch_key];
    layer_idx = layer_flat?.has[type_id] ? layer_flat.index[type_id] : NO_LAYER;
  }
  return { material_class, r, g, b, layer_idx };
}

/**
 * One band-aligned floor shared by the whole chunk. Unlike the old single
 * 64-high band, sparse signed-Z bands can extend through every actual
 * peak, so no z_top clamp can cut mountainous terrain or raycasts flat.
 */
function terrain_floor_of(ele: I16Array): number {
  let min_ele = Infinity;
  for (let i = 0; i < CHUNK_CELL_SPAN2; i++) min_ele = Math.min(min_ele, ele[i]);
  return Math.floor((min_ele - TERRAIN_Z_MARGIN) / L2_BAND_CELL_SPAN) * L2_BAND_CELL_SPAN;
}

export class TerrainWorldLayer implements WorldLayer {
  readonly name = "terrain";

  revision(key: ChunkKey): WorldLayerRevision {
    return [useTileStore.getState().get_tile(key.page_idx), useSettingsStore.getState().show_structures];
  }

  /**
   * One run per column, floor to elevation, in the column's own material —
   * the same columns paint() fills into the volume, without the volume. See
   * WorldLayer.runs.
   */
  runs(key: ChunkKey, w: ColumnRunWriter): boolean {
    const tile = useTileStore.getState().get_tile(key.page_idx);
    if (!tile) return false;

    const { ground, ele } = slice_chunk(tile.terrain.ground, tile.terrain.ele, key.page_off);
    const show_structures = useSettingsStore.getState().show_structures;
    const terrain_floor = terrain_floor_of(ele);

    for (let i: u32 = 0; i < CHUNK_CELL_SPAN2; i++) {
      w.column(i);
      const top = ele[i] - 1;
      if (top < terrain_floor) continue;
      w.run(terrain_floor, top, ground_material_at(ground, i, show_structures));
    }
    return true;
  }

  paint(key: ChunkKey, target: ChunkVolume): boolean {
    const tile = useTileStore.getState().get_tile(key.page_idx);
    if (!tile) return false;

    const { ground, ele } = slice_chunk(tile.terrain.ground, tile.terrain.ele, key.page_off);
    const show_structures = useSettingsStore.getState().show_structures;
    const terrain_floor = terrain_floor_of(ele);

    for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
      for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
        const i = cy * CHUNK_CELL_SPAN + cx;
        const material = ground_material_at(ground, i, show_structures);
        fill_chunk_column(target, cx, cy, terrain_floor, ele[i] - 1, material);
      }
    }
    return true;
  }
}
