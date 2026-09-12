// @pxl/web/modules/world/surface_height.ts

import type { f64, u32 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L2_BASE } from "@pxl/shared/spec/space/indexer.ts";
import { TO_XY } from "@pxl/shared/spec/space/xy.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { get_ele_at_local } from "./terrain/elevation.ts";
import { local_world_to_canonical } from "./frame/coord.ts";
import { each_column_entry } from "./objects/store.ts";
import { object_value_is_air } from "@pxl/shared/spec/cell/mod.ts";

// Two Z values within this of each other count as "flush" — guards the
// climb loop against re-finding the object that just set `surface` on the
// next pass, and the "is this actually resting there" / "is this at or
// past the hint" checks below, against float noise from the same PVF3
// round-trip conversions object_local_corner's own OVERLAP_EPS guards
// against elsewhere.
const SAME_SURFACE_EPS: f64 = 1e-6;

/**
 * The z extents of every placed object whose footprint covers a local XY,
 * gathered in one pass over the chunk columns that could hold one.
 *
 * This replaces probing upward a cell at a time with a 27-cell neighbourhood
 * query at each step: with nothing placed at the XY the probe loop ran to
 * its 64-cell guard every call, and the companion asked twice a frame —
 * 4.8 ms a frame on a phone, all of it BigInt cell keys and the arrays each
 * query built (the step table, 2026-09-13). A placed object is at most a
 * cell across, so one whose footprint covers the point lives in the chunk
 * the point is in or a chunk within a cell of it; those are read once, with
 * the store's own cursor, and nothing is allocated.
 */
const $probe: V3 = [0, 0, 0];
const $corner: PVF3 = { i: 0n, f: 0n };
let $min_z = new Float64Array(64);
let $max_z = new Float64Array(64);
const $bases: IVec[] = [];

function push_box(min_z: f64, max_z: f64, n: u32): void {
  if (n >= $min_z.length) {
    const grow = (a: Float64Array) => {
      const b = new Float64Array(a.length * 2);
      b.set(a);
      return b;
    };
    $min_z = grow($min_z);
    $max_z = grow($max_z);
  }
  $min_z[n] = min_z;
  $max_z[n] = max_z;
}

function collect_boxes(map_origin: IVec, local_x: f64, local_y: f64): u32 {
  let n: u32 = 0;
  $bases.length = 0;
  for (let dx = -1; dx <= 1; dx++) {
    for (let dy = -1; dy <= 1; dy++) {
      $probe[0] = local_x + dx * U_PER_CELL;
      $probe[1] = local_y + dy * U_PER_CELL;
      $probe[2] = 0;
      const base = L2_BASE(TO_XY(local_world_to_canonical($corner, $probe, map_origin).i));
      if ($bases.includes(base)) continue;
      $bases.push(base);
      // The chunk's corner in the local frame; entries are relative to it.
      const local = { i: ivec.sub(base, map_origin), f: 0n };
      const bx = pvf3.to_real(pvf3.X(local)), by = pvf3.to_real(pvf3.Y(local)), bz = pvf3.to_real(pvf3.Z(local));
      each_column_entry(base, (entry) => {
        if (object_value_is_air(entry.state)) return;
        const x0 = bx + entry.lx, y0 = by + entry.ly;
        if (local_x < x0 || local_x >= x0 + entry.size || local_y < y0 || local_y >= y0 + entry.size) return;
        const z0 = bz + entry.lz;
        push_box(z0, z0 + entry.size, n++);
      });
    }
  }
  return n;
}

/**
 * Ground height at a local XY point, accounting for placed objects sitting
 * on top of terrain there — not just the raw terrain elevation
 * get_ele_at_local alone reports. Companion needs this: walking onto a
 * placed block (or a stack of them) should stand on top, the same way
 * tapping a hover cursor on one aligns map_center's own Z to it
 * (camera/input.tsx's handle_tap -> objects/geometry.ts's object_center).
 * That existing path is itself known-fragile (its own comment: a drag
 * right afterward snaps the camera target straight back down to terrain,
 * "not fixed here") — this is a from-scratch position-based query instead
 * of reusing that Z, since the character's Z has to track its own current
 * XY as it walks, not wherever map_center currently is.
 *
 * Thin wrapper over surface_z_from_terrain: the only tile-store-dependent
 * part of this query is the terrain lookup itself, split out here so the
 * actual gap/climb algorithm (surface_height.test.ts's real target) can be
 * exercised headlessly against synthetic terrain heights, with no
 * useTileStore data required.
 */
export function get_surface_z_at_local(
  origin_page_idx_xy: IVec,
  map_origin: IVec,
  local_x: f64,
  local_y: f64,
  hint_z: f64,
): f64 | null {
  const terrain_ele = get_ele_at_local(origin_page_idx_xy, local_x, local_y);
  if (terrain_ele === null) return null;
  return surface_z_from_terrain(map_origin, local_x, local_y, terrain_ele, hint_z);
}

/**
 * `hint_z` is roughly where the current navigation target is vertically
 * (map_center's own Z is what companion.tsx passes) — used only to pick
 * *which* shelf to land on when the stack at this XY has a gap in it:
 * scans upward from the hint for the lowest object whose top is at or
 * above it, i.e. snaps up past a gap onto the next real platform rather
 * than always defaulting to the first one reachable from the ground. Once
 * landed, still climbs any further objects resting flush on top of that
 * shelf, so the character ends up on the true top of whatever contiguous
 * structure it's standing on, not wherever exactly the hint happened to
 * land inside it. Falls back to climbing from the ground when nothing
 * qualifies above the hint (open sky above an unrelated structure, or no
 * hint worth trusting).
 */
export function surface_z_from_terrain(
  map_origin: IVec,
  local_x: f64,
  local_y: f64,
  terrain_ele: f64,
  hint_z: f64,
): f64 {
  const n = collect_boxes(map_origin, local_x, local_y);
  if (n === 0) return terrain_ele;

  // Snap-up: the lowest top at or above the hint, among boxes from the cell
  // below where the search starts — what stepping upward from there found.
  const search_from = Math.max(hint_z, terrain_ele);
  const floor_z = (Math.floor(search_from / U_PER_CELL) - 1) * U_PER_CELL;
  let landing: f64 | null = null;
  for (let b = 0; b < n; b++) {
    if ($min_z[b] < floor_z) continue;
    if ($max_z[b] < hint_z - SAME_SURFACE_EPS) continue;
    if (landing === null || $max_z[b] < landing) landing = $max_z[b];
  }

  let surface = landing ?? terrain_ele;

  // Climb any further objects resting flush on top of the landing spot —
  // the whole point is standing on the *true* top of whatever contiguous
  // structure is under the character, not wherever the snap-up search (or
  // the ground, in the fallback case) happened to land. Each pass lifts the
  // surface past at least one box, so it ends within the box count.
  for (let pass = 0; pass < n; pass++) {
    let top: f64 | null = null;
    for (let b = 0; b < n; b++) {
      if ($min_z[b] > surface + SAME_SURFACE_EPS) continue;
      if ($max_z[b] <= surface + SAME_SURFACE_EPS) continue;
      if (top === null || $max_z[b] > top) top = $max_z[b];
    }
    if (top === null) break;
    surface = top;
  }

  return surface;
}
