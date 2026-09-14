// @pxl/web/reference/terrain_layer.ts

/**
 * L3 heightfield -> temporary dense signed-Z render bands, and the direct
 * path's runs.
 *
 * The chunk is read by the module (crates/pxl/core/src/bake/terrain.rs): the
 * page's ground and elevation lanes are in the core memory, so it samples
 * and paints them where they are, with the tables handed to it once here.
 * A page whose lanes spilled past the page section (core_pool.ts) is out of
 * the module's reach and is read the TypeScript way below — the same cells,
 * held byte for byte to the module's by terrain.test.ts.
 */

import type { u32 } from "@xpute/core/kit/word.ts";
import type { I16Array } from "@xpute/core/kit/array.ts";

import { GROUND_CH_KEYS, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import { cube_types } from "@pxl/shared/spec/cell/cube.ts";

import { useSettingsStore } from "@/app/setting/store.ts";
import { useTileStore } from "@/module/world/terrain/store.ts";
import { slice_chunk } from "@/reference/page_slice.ts";
import { GROUND_COLOR_LOOKUP_FLAT, GROUND_LAYER_INDEX_LOOKUP_FLAT } from "@/render/gpu/layer_table.ts";
import { BASE_B, BASE_G, BASE_LAYER_IDX, BASE_R, CH_TO_CLASS, teach_terrain } from "@/module/world/terrain/teach.ts";

import { type ChunkVolume, fill_chunk_column, L2_BAND_CELL_SPAN, note_paint } from "@/reference/volume.ts";
import { MaterialClass, NO_LAYER, type VoxelMaterial } from "@/render/mesh/voxel.ts";
import type { ColumnRunWriter } from "@/reference/voxel_scan.ts";
import { CHUNK_CELL_SPAN } from "@pxl/shared/bake/constant.ts";
import type { ChunkKey, WorldLayer } from "@/reference/contract.ts";
import { CORE_PXL } from "@/wasm/core_pool.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { CELL_SHAMT } from "@pxl/shared/bake/constant.ts";
import type { Terrain } from "@pxl/shared/bake/format.ts";

const CHUNK_CELL_SPAN2 = CHUNK_CELL_SPAN * CHUNK_CELL_SPAN;
const TERRAIN_Z_MARGIN = 4;

// The tests run the reference against the module: the module has to know
// the tables the reference reads.
teach_terrain();

type RenderGround = ReturnType<typeof slice_chunk>["ground"];

/** The one material object every column's answer is written into: the two
 * callers below consume it before asking for the next column, so nothing
 * is kept and nothing is allocated — this was one object per column, four
 * thousand per chunk. */
const MATERIAL: VoxelMaterial = { material_class: MaterialClass.LAND, r: 0, g: 0, b: 0, layer_idx: NO_LAYER };

/** The TypeScript reading of a cell's material — the reference the
 * module's is held to, and the path for a page the module cannot reach. */
export function ground_material_at(ground: RenderGround, idx: u32, show_structures: boolean): VoxelMaterial {
  const m = MATERIAL;
  m.r = BASE_R;
  m.g = BASE_G;
  m.b = BASE_B;
  m.material_class = MaterialClass.LAND;
  m.layer_idx = BASE_LAYER_IDX;

  for (const ch_key of GROUND_CH_KEYS) {
    if (ch_key === GroundChannelKey.STRUCTURE && !show_structures) continue;
    const type_id = ground[ch_key][idx];
    if (type_id === cube_types.common.VOID) continue;
    const flat = GROUND_COLOR_LOOKUP_FLAT[ch_key];
    if (!flat?.has[type_id]) continue;

    const off = type_id * 3;
    m.r = flat.rgb[off];
    m.g = flat.rgb[off + 1];
    m.b = flat.rgb[off + 2];
    m.material_class = CH_TO_CLASS[ch_key];
    const layer_flat = GROUND_LAYER_INDEX_LOOKUP_FLAT[ch_key];
    m.layer_idx = layer_flat?.has[type_id] ? layer_flat.index[type_id] : NO_LAYER;
  }
  return m;
}

/**
 * One band-aligned floor shared by the whole chunk. Unlike the old single
 * 64-high band, sparse signed-Z bands can extend through every actual
 * peak, so no z_top clamp can cut mountainous terrain or raycasts flat.
 */
export function terrain_floor_of(ele: I16Array): number {
  let min_ele = Infinity;
  for (let i = 0; i < CHUNK_CELL_SPAN2; i++) min_ele = Math.min(min_ele, ele[i]);
  return Math.floor((min_ele - TERRAIN_Z_MARGIN) / L2_BAND_CELL_SPAN) * L2_BAND_CELL_SPAN;
}

/** Whether the module can read every lane of `terrain` where it is. */
function module_reaches(terrain: Terrain): boolean {
  if (CORE_PXL === null) return false;
  const memory = CORE_PXL.memory.buffer;
  return terrain.ele.buffer === memory && GROUND_CH_KEYS.every((k) => terrain.ground[k].buffer === memory);
}

/** Reads the chunk at `page_off` of `terrain` into the module: its floor. */
function module_read(terrain: Terrain, page_off: IVec, show_structures: boolean): number {
  const g = terrain.ground;
  const tx0: u32 = Number(ivec.X(page_off)) << -CELL_SHAMT;
  const ty0: u32 = Number(ivec.Y(page_off)) << -CELL_SHAMT;
  return CORE_PXL!.pxl_terrain_read(g.land.byteOffset, g.water.byteOffset, g.structure.byteOffset, g.transport.byteOffset, terrain.ele.byteOffset, tx0, ty0, show_structures ? 1 : 0);
}

export class TerrainWorldLayer implements WorldLayer {
  readonly name = "terrain";

  paint(key: ChunkKey, target: ChunkVolume): boolean {
    const tile = useTileStore.getState().get_tile(key.page_idx);
    if (!tile) return false;
    // A page the module cannot read has nothing to paint; the chunks built
    // from it wait for it in the module the same way (core_build.ts).
    if (!module_reaches(tile.terrain)) return false;
    paint_module(tile.terrain, key.page_off, useSettingsStore.getState().show_structures, target);
    return true;
  }
}

/** The module's paint of one chunk: read, then filled column by column
 * there. The terrain must be within the module's reach. */
export function paint_module(terrain: Terrain, page_off: IVec, show_structures: boolean, target: ChunkVolume): void {
  module_read(terrain, page_off, show_structures);
  note_paint(target, CORE_PXL!.pxl_terrain_paint(target.id));
}

/** The module's reading of one chunk written as the direct path's runs:
 * one a column, floor to top, in the cell's own material. */
export function runs_module(terrain: Terrain, page_off: IVec, show_structures: boolean, w: ColumnRunWriter): void {
  const p = CORE_PXL!;
  const floor = module_read(terrain, page_off, show_structures);
  const material = new Uint32Array(p.memory.buffer, p.pxl_terrain_material_at(), CHUNK_CELL_SPAN2);
  const layer = new Uint8Array(p.memory.buffer, p.pxl_terrain_layer_at(), CHUNK_CELL_SPAN2);
  const ele = new Int16Array(p.memory.buffer, p.pxl_terrain_ele_at(), CHUNK_CELL_SPAN2);
  const m = MATERIAL;
  for (let i: u32 = 0; i < CHUNK_CELL_SPAN2; i++) {
    w.column(i);
    const top = ele[i] - 1;
    if (top < floor) continue;
    const packed = material[i];
    m.material_class = (packed >>> 24) as MaterialClass;
    m.r = (packed >>> 16) & 255;
    m.g = (packed >>> 8) & 255;
    m.b = packed & 255;
    m.layer_idx = layer[i];
    w.run(floor, top, m);
  }
}

/** The TypeScript direct path: the reference, and the path for a page the
 * module cannot reach. */
export function runs_typescript(terrain: Terrain, page_off: IVec, show_structures: boolean, w: ColumnRunWriter): void {
  const { ground, ele } = slice_chunk(terrain.ground, terrain.ele, page_off);
  const terrain_floor = terrain_floor_of(ele);
  for (let i: u32 = 0; i < CHUNK_CELL_SPAN2; i++) {
    w.column(i);
    const top = ele[i] - 1;
    if (top < terrain_floor) continue;
    w.run(terrain_floor, top, ground_material_at(ground, i, show_structures));
  }
}

/** The TypeScript paint: the reference the module's is held to, and the
 * path for a page the module cannot reach. */
export function paint_typescript(terrain: Terrain, page_off: IVec, show_structures: boolean, target: ChunkVolume): void {
  const { ground, ele } = slice_chunk(terrain.ground, terrain.ele, page_off);
  const terrain_floor = terrain_floor_of(ele);
  for (let cy: u32 = 0; cy < CHUNK_CELL_SPAN; cy++) {
    for (let cx: u32 = 0; cx < CHUNK_CELL_SPAN; cx++) {
      const i = cy * CHUNK_CELL_SPAN + cx;
      const material = ground_material_at(ground, i, show_structures);
      fill_chunk_column(target, cx, cy, terrain_floor, ele[i] - 1, material);
    }
  }
}
