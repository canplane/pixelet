// @pxl/web/modules/world/terrain/use-elevation.ts

import { useTileStore } from "./tile-store.ts";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { type FVec, type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L3, L3_IDX, L3_RANGE } from "@pxl/shared/spec/space/indexer.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import type { Tile } from "@pxl/shared/bake/format.ts";
import { sample_ele } from "@/render/terrain/elevation.ts";

const L3_RANGE_REAL: f64 = Number(L3_RANGE);

/** Returns ground height in lattice/render units. */
export function get_ele(i: IVec, f: FVec): f64 | null {
  const get_tile = useTileStore.getState().get_tile;

  const page_idx: IVec = L3_IDX(i);
  const tile: Tile | undefined = get_tile(page_idx);
  if (!tile) return null;

  const page_off: PVF3 = { i: L1_IDX_IN_L3(i), f };
  const tx: u32 = Math.floor(pvf3.to_real(pvf3.X(page_off)) / U_PER_CELL);
  const ty: u32 = Math.floor(pvf3.to_real(pvf3.Y(page_off)) / U_PER_CELL);
  return sample_ele(tile.terrain.ele, tx, ty) * U_PER_CELL;
}

/**
 * Fast path for hot per-step loops (e.g. raycasting): looks up elevation from
 * local float coordinates already relative to map_origin (same real "u" units
 * as pvf3.to_real), instead of round-tripping through the full BigInt PVF3
 * fixed-point representation on every call. Only the page-key lookup itself
 * still touches BigInt, and only for one (typically -1/0/1) candidate page.
 *
 * `origin_page_idx_xy` must be TO_XY(L3_IDX(map_origin)), precomputed once by
 * the caller outside the loop. map_origin is L4-aligned, which is coarser
 * than (and therefore also aligned to) L3, so page indices compose
 * additively: L3_IDX(local + origin) === L3_IDX(local) + L3_IDX(origin).
 */
export function get_ele_at_local(origin_page_idx_xy: IVec, local_x: f64, local_y: f64): f64 | null {
  const get_tile = useTileStore.getState().get_tile;

  const local_page_x: f64 = Math.floor(local_x / L3_RANGE_REAL);
  const local_page_y: f64 = Math.floor(local_y / L3_RANGE_REAL);

  const page_idx: IVec = ivec.add(origin_page_idx_xy, ivec.gather(BigInt(local_page_x), BigInt(local_page_y), 0n));
  const tile: Tile | undefined = get_tile(page_idx);
  if (!tile) return null;

  const page_local_x: f64 = local_x - local_page_x * L3_RANGE_REAL;
  const page_local_y: f64 = local_y - local_page_y * L3_RANGE_REAL;

  const tx: u32 = Math.floor(page_local_x / U_PER_CELL);
  const ty: u32 = Math.floor(page_local_y / U_PER_CELL);
  return sample_ele(tile.terrain.ele, tx, ty) * U_PER_CELL;
}

export function with_ele(out: PVF3, i: IVec, f: FVec): PVF3 | null {
  // Sample terrain height at the current X/Y position.
  const ele: f64 | null = get_ele(i, f);
  if (ele === null) return null;

  // Rebuild absolute PVF3 with ground-relative Z applied.
  out.i = i;
  out.f = f;
  const grounded_z = pvf3.Z(out) + pvf3.from_real(ele);
  pvf3.gather(out, pvf3.X(out), pvf3.Y(out), grounded_z);
  return out;
}

// export function useEleAt(i: IVec, f: FVec): f64 | null {
//   const page_idx: IVec = L3_IDX(i);

//   const tile = useTileStore((state) => state.get_tile(page_idx));

//   return useMemo(() => {
//     if (!tile) return null;

//     const page_off: PVF3 = { i: L1_IDX_IN_L3(i), f };
//     const tx: u32 = Math.floor(pvf3.to_real(pvf3.X(page_off)) / U_PER_CELL);
//     const ty: u32 = Math.floor(pvf3.to_real(pvf3.Y(page_off)) / U_PER_CELL);
//     return sample_ele(tile.terrain.ele, tx, ty) * U_PER_CELL;
//   }, [tile, i, f]);
// }
