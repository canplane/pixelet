// @pxl/web/modules/world/terrain/use-elevation.ts

import { useTileStore } from "./tile-store.ts";

import type { f64, u32 } from "@xpute/spec/lib/kit/word.ts";

import { type FVec, type IVec, type PVF3, pvf3 } from "@pxl/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L3, L3_IDX } from "@pxl/spec/space/indexer.ts";
import { U_PER_CELL } from "@pxl/bake/terrain/constants.ts";
import type { Tile } from "@pxl/bake/terrain/format.ts";
import { sample_ele } from "@pxl/render/terrain/elevation.ts";

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

/** Returns ground height in lattice/render units. */
export function get_ele(i: IVec, f: FVec): f64 | null {
  const { get_tile } = useTileStore.getState();

  const page_idx: IVec = L3_IDX(i);
  const tile: Tile | undefined = get_tile(page_idx);
  if (!tile) return null;

  const page_off: PVF3 = { i: L1_IDX_IN_L3(i), f };
  const tx: u32 = Math.floor(pvf3.to_real(pvf3.X(page_off)) / U_PER_CELL);
  const ty: u32 = Math.floor(pvf3.to_real(pvf3.Y(page_off)) / U_PER_CELL);
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
