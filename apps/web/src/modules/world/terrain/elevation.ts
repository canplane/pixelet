// @pxl/web/modules/world/terrain/elevation.ts

import { useTileStore } from "./store.ts";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { type FVec, type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L1_IDX_IN_L3, L3_IDX, L3_RANGE } from "@pxl/shared/spec/space/indexer.ts";
import { PAGE_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import type { GroundChannelKey, Terrain, Tile } from "@pxl/shared/bake/format.ts";
import { is_water_family_at, sample_ele, WATER_RECESS_CELLS } from "@/render/layer/terrain/elevation.ts";

const L3_RANGE_REAL: f64 = Number(L3_RANGE);

/** Same real height chunk.ts's slice_chunk bakes into the mesh/raycast path
 * (elevation.ts's WATER_RECESS_CELLS) — applied here too so this module's
 * own consumers (camera ground-snap, ground_or_raw, any future cell-marker
 * probe) agree with the rendered terrain instead of reasoning about the
 * un-recessed height while the mesh they're looking at shows the lowered
 * one. `tile.terrain.ele`/`.ground` are both page-level here (unsliced,
 * unlike chunk.ts's chunk-local copies), so tx/ty index them directly at
 * PAGE_CELL_SPAN stride. */
function sample_ele_grounded(terrain: Terrain, tx: u32, ty: u32): number {
  const ele = sample_ele(terrain.ele, tx, ty);
  return is_water_family_at(terrain.ground, ty * PAGE_CELL_SPAN + tx) ? ele - WATER_RECESS_CELLS : ele;
}

/** Returns ground height in lattice/render units. */
export function get_ele(i: IVec, f: FVec): f64 | null {
  const get_tile = useTileStore.getState().get_tile;

  const page_idx: IVec = L3_IDX(i);
  const tile: Tile | undefined = get_tile(page_idx);
  if (!tile) return null;

  const page_off: PVF3 = { i: L1_IDX_IN_L3(i), f };
  const tx: u32 = Math.floor(pvf3.to_real(pvf3.X(page_off)) / U_PER_CELL);
  const ty: u32 = Math.floor(pvf3.to_real(pvf3.Y(page_off)) / U_PER_CELL);
  return sample_ele_grounded(tile.terrain, tx, ty) * U_PER_CELL;
}

/** Raw baked ground-channel value at this exact cell, one specific channel
 * (LAND/WATER/STRUCTURE/TRANSPORT) — get_ele's own sibling, same tile/tx/ty
 * lookup, just reading `.ground[channel]` instead of sampling `.ele`. Used
 * by objects/store.ts's place() to tell whether a terrain-styled pick
 * (cell/mod.ts's own TERRAIN_STYLE_SOURCE) would actually change anything —
 * painting "land.GREEN_DEEP" over ground the bake already recorded as
 * GREEN_DEEP land paints nothing new. */
export function get_ground_type(i: IVec, f: FVec, channel: GroundChannelKey): number | null {
  const get_tile = useTileStore.getState().get_tile;

  const page_idx: IVec = L3_IDX(i);
  const tile: Tile | undefined = get_tile(page_idx);
  if (!tile) return null;

  const page_off: PVF3 = { i: L1_IDX_IN_L3(i), f };
  const tx: u32 = Math.floor(pvf3.to_real(pvf3.X(page_off)) / U_PER_CELL);
  const ty: u32 = Math.floor(pvf3.to_real(pvf3.Y(page_off)) / U_PER_CELL);
  return tile.terrain.ground[channel][ty * PAGE_CELL_SPAN + tx];
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

  // the overwhelmingly common case per-step (a ray mostly stays within the
  // origin page) — skip the BigInt gather/add round-trip entirely instead
  // of doing it just to compute an identity.
  const page_idx: IVec = local_page_x === 0 && local_page_y === 0 ? origin_page_idx_xy : ivec.add(origin_page_idx_xy, ivec.gather(BigInt(local_page_x), BigInt(local_page_y), 0n));
  const tile: Tile | undefined = get_tile(page_idx);
  if (!tile) return null;

  const page_local_x: f64 = local_x - local_page_x * L3_RANGE_REAL;
  const page_local_y: f64 = local_y - local_page_y * L3_RANGE_REAL;

  const tx: u32 = Math.floor(page_local_x / U_PER_CELL);
  const ty: u32 = Math.floor(page_local_y / U_PER_CELL);
  return sample_ele_grounded(tile.terrain, tx, ty) * U_PER_CELL;
}

export function with_ele(out: PVF3, i: IVec, f: FVec): PVF3 | null {
  // sample terrain height at the current X/Y position.
  const ele: f64 | null = get_ele(i, f);
  if (ele === null) return null;

  // rebuild absolute PVF3 with ground-relative Z applied.
  out.i = i;
  out.f = f;
  const grounded_z = pvf3.Z(out) + pvf3.from_real(ele);
  pvf3.gather(out, pvf3.X(out), pvf3.Y(out), grounded_z);
  return out;
}

/** Half a cell, in the fixed-point Z a PVF3 carries. */
const HALF_CELL_FP = pvf3.from_real(U_PER_CELL / 2);

/**
 * Where the camera's pivot sits on the ground at a coordinate's X/Y: the
 * centre of the cube standing on the ground there — the cell the character
 * occupies, the cell a placement's ghost fills — its original Z discarded.
 * Null when no elevation is loaded yet, like with_ele.
 *
 * A centre, not the top face, because everything the camera pivots on is a
 * cube: a tap on an object pivots on that cube's centre
 * (objects/geometry.ts's object_center), and a tap on the ground on the
 * centre of the cube above it (camera/input.tsx). A pivot that merely sits
 * on the ground has to agree with those, or the first pan after a tap
 * moves the camera by the half cell between the two. with_ele itself
 * answers the top face, which is what standing things (the puck, decor, a
 * place) are grounded to; this is the one place the camera's own
 * convention is applied.
 */
export function ground_pivot(out: PVF3, i: IVec, f: FVec): PVF3 | null {
  const grounded = with_ele(out, i, f);
  if (grounded === null) return null;
  pvf3.gather(out, pvf3.X(out), pvf3.Y(out), pvf3.Z(out) + HALF_CELL_FP);
  return out;
}

/**
 * ground_pivot for a coordinate, falling back to the coordinate itself when
 * no elevation is loaded yet.
 *
 * The single shared "follow the actual ground" step any map_center-setting
 * interaction should use, instead of each call site hand-rolling its own
 * zero-Z-then-with_ele probe — do_pan and the GPS recenter button used to
 * each build this independently (the recenter button skipped it entirely,
 * setting map_center to the device's raw GPS altitude instead of real
 * terrain height, visibly disagreeing on "where the ground is" depending on
 * which interaction moved the camera).
 */
export function ground_or_raw(coord: PVF3): PVF3 {
  const probe: PVF3 = { i: 0n, f: 0n };
  pvf3.gather(probe, pvf3.X(coord), pvf3.Y(coord), 0n);
  return ground_pivot(probe, probe.i, probe.f) ?? coord;
}
