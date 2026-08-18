// @pxl/web/render/layer/pipeline/voxel_scan.ts

/** Shared band scan used by rendering and raycasting. */

import type { i32, u32 } from "@xpute/core/kit/word.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import {
  band_voxel_idx,
  type ChunkVolume,
  get_chunk_band,
  L2_BAND_CELL_MASK,
  L2_BAND_CELL_SPAN,
  MaterialClass,
  ordered_chunk_bands,
  SUB_CELL_NONE,
  type SubVoxelEntry,
  unpack_sub_cell_fraction,
  type VoxelBand,
} from "./volume.ts";

export type MeshRun = {
  z0: i32;
  z1: i32;
  r: u32;
  g: u32;
  b: u32;
  layer_idx: u32;
  material_class: MaterialClass;
  sub_sz: u32;
  sub_ox: u32;
  sub_oy: u32;
  sub_oz: u32;
};

function push_sub_voxel_run(runs: MeshRun[], entry: SubVoxelEntry): void {
  runs.push({
    z0: entry.z,
    z1: entry.z,
    r: entry.r,
    g: entry.g,
    b: entry.b,
    layer_idx: entry.layer_idx,
    material_class: entry.material_class,
    sub_sz: entry.sub_sz,
    sub_ox: entry.sub_ox,
    sub_oy: entry.sub_oy,
    sub_oz: entry.sub_oz,
  });
}

/**
 * Per-(band, column) buckets of `extra_sub_voxels`, built once and reused
 * across every column `merge_column_runs` visits for the same chunk
 * (`scan_all_column_runs` below) — filtering a band's `extra_sub_voxels` by
 * `cx`/`cy` fresh on every one of those 4096 calls would be an O(4096 * N)
 * re-scan of the same array, dominating cost on a chunk with many placed
 * sub-cell objects (large N).
 */
export type SubVoxelIndex = ReadonlyMap<VoxelBand, ReadonlyMap<u32, readonly SubVoxelEntry[]>>;

export function index_extra_sub_voxels(bands: readonly VoxelBand[]): SubVoxelIndex {
  const index = new Map<VoxelBand, Map<u32, SubVoxelEntry[]>>();
  for (const band of bands) {
    if (band.extra_sub_voxels.length === 0) continue;
    const per_column = new Map<u32, SubVoxelEntry[]>();
    for (const entry of band.extra_sub_voxels) {
      const key = entry.cy * L2_BAND_CELL_SPAN + entry.cx;
      let list = per_column.get(key);
      if (!list) per_column.set(key, list = []);
      list.push(entry);
    }
    index.set(band, per_column);
  }
  return index;
}

export function merge_column_runs(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  only_band_z?: i32,
  sub_voxel_index?: SubVoxelIndex,
): MeshRun[] {
  const runs: MeshRun[] = [];
  if (volume.bands.length === 0) return runs;

  const band_x = Math.floor(cx / L2_BAND_CELL_SPAN);
  const band_y = Math.floor(cy / L2_BAND_CELL_SPAN);
  const local_x = cx & L2_BAND_CELL_MASK;
  const local_y = cy & L2_BAND_CELL_MASK;
  let open: MeshRun | null = null;

  const ordered = only_band_z === undefined ? ordered_chunk_bands(volume) : (() => {
    const band = get_chunk_band(volume, band_x, band_y, only_band_z);
    return band ? [band] : [];
  })();
  let previous_band_z: i32 | null = null;
  for (const band of ordered) {
    const band_z = band.band_z;
    // Empty signed-Z bands break a material run. Adjacent bands do not:
    // buried faces at a cache boundary must not reappear in the mesh.
    if (previous_band_z !== null && band_z !== previous_band_z + 1) open = null;
    previous_band_z = band_z;

    for (let local_z = 0; local_z < L2_BAND_CELL_SPAN; local_z++) {
      const idx = band_voxel_idx(local_x, local_y, local_z);
      const packed = band.material[idx];
      if (packed === 0) {
        open = null;
        continue;
      }

      const z = band_z * L2_BAND_CELL_SPAN + local_z;
      const r = (packed >>> 16) & 255, g = (packed >>> 8) & 255, b = packed & 255;
      const material_class = (packed >>> 24) as MaterialClass;
      const layer_idx = band.layer_idx[idx];
      const so = idx * 4;
      const sub_sz = band.sub_cell?.[so + 3] ?? SUB_CELL_NONE;

      if (sub_sz !== SUB_CELL_NONE) {
        runs.push({
          z0: z,
          z1: z,
          r,
          g,
          b,
          layer_idx,
          material_class,
          sub_sz,
          sub_ox: band.sub_cell![so],
          sub_oy: band.sub_cell![so + 1],
          sub_oz: band.sub_cell![so + 2],
        });
        open = null;
      } else if (
        open && material_class !== MaterialClass.OBJECT && open.r === r && open.g === g && open.b === b &&
        open.layer_idx === layer_idx && open.material_class === material_class
      ) {
        open.z1 = z;
      } else {
        open = { z0: z, z1: z, r, g, b, layer_idx, material_class, sub_sz: SUB_CELL_NONE, sub_ox: 0, sub_oy: 0, sub_oz: 0 };
        runs.push(open);
      }
    }

    if (sub_voxel_index) {
      const entries = sub_voxel_index.get(band)?.get(cy * L2_BAND_CELL_SPAN + cx);
      if (entries) { for (const entry of entries) push_sub_voxel_run(runs, entry); }
    } else {
      for (const entry of band.extra_sub_voxels) {
        if (entry.cx !== cx || entry.cy !== cy) continue;
        push_sub_voxel_run(runs, entry);
      }
    }
  }
  return runs;
}

export function topmost_voxel(
  volume: ChunkVolume,
  cx: u32,
  cy: u32,
  only_band_z?: i32,
): { z: i32; r: u32; g: u32; b: u32; layer_idx: u32 } | null {
  if (volume.bands.length === 0) return null;
  const band_x = Math.floor(cx / L2_BAND_CELL_SPAN);
  const band_y = Math.floor(cy / L2_BAND_CELL_SPAN);
  const local_x = cx & L2_BAND_CELL_MASK;
  const local_y = cy & L2_BAND_CELL_MASK;

  if (only_band_z !== undefined) {
    const band = get_chunk_band(volume, band_x, band_y, only_band_z);
    return band ? topmost_in_band(band, local_x, local_y, only_band_z) : null;
  }
  const ordered = ordered_chunk_bands(volume);
  for (let i = ordered.length - 1; i >= 0; i--) {
    const band = ordered[i];
    const band_z = band.band_z;
    const top = topmost_in_band(band, local_x, local_y, band_z);
    if (top) return top;
  }
  return null;
}

function topmost_in_band(
  band: VoxelBand,
  local_x: u32,
  local_y: u32,
  band_z: i32,
): { z: i32; r: u32; g: u32; b: u32; layer_idx: u32 } | null {
  for (let local_z = L2_BAND_CELL_SPAN - 1; local_z >= 0; local_z--) {
    const idx = band_voxel_idx(local_x, local_y, local_z);
    const packed = band.material[idx];
    if (packed === 0) continue;
    return {
      z: band_z * L2_BAND_CELL_SPAN + local_z,
      r: (packed >>> 16) & 255,
      g: (packed >>> 8) & 255,
      b: packed & 255,
      layer_idx: band.layer_idx[idx],
    };
  }
  return null;
}

/**
 * Every column's runs, scanned once. `merge_column_runs` walks live band
 * data (`ChunkVolume`), which the renderer only keeps alive for one
 * synchronous composite pass — this copies each column's result out into
 * plain `MeshRun` objects (already true of `merge_column_runs`'s own
 * output, never a view into the volume's typed arrays) so callers can
 * retain and reuse `MeshRun[][]` across LOD rebuilds long after that
 * volume has been released back to its pool.
 */
export function scan_all_column_runs(volume: ChunkVolume): MeshRun[][] {
  const sub_voxel_index = index_extra_sub_voxels(volume.bands);
  const out: MeshRun[][] = new Array(L2_BAND_CELL_SPAN * L2_BAND_CELL_SPAN);
  for (let cy: u32 = 0; cy < L2_BAND_CELL_SPAN; cy++) {
    for (let cx: u32 = 0; cx < L2_BAND_CELL_SPAN; cx++) {
      out[cy * L2_BAND_CELL_SPAN + cx] = merge_column_runs(volume, cx, cy, undefined, sub_voxel_index);
    }
  }
  return out;
}

export type RunBox = { min: V3; max: V3 };

export function run_to_box(cx: u32, cy: u32, run: MeshRun): RunBox {
  if (run.sub_sz === SUB_CELL_NONE) {
    return {
      min: [cx * U_PER_CELL, cy * U_PER_CELL, run.z0 * U_PER_CELL],
      max: [(cx + 1) * U_PER_CELL, (cy + 1) * U_PER_CELL, (run.z1 + 1) * U_PER_CELL],
    };
  }

  const sz = unpack_sub_cell_fraction(run.sub_sz) * U_PER_CELL;
  const min: V3 = [
    (cx + unpack_sub_cell_fraction(run.sub_ox)) * U_PER_CELL,
    (cy + unpack_sub_cell_fraction(run.sub_oy)) * U_PER_CELL,
    (run.z0 + unpack_sub_cell_fraction(run.sub_oz)) * U_PER_CELL,
  ];
  return { min, max: [min[0] + sz, min[1] + sz, min[2] + sz] };
}
