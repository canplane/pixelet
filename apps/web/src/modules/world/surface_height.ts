// @pxl/web/modules/world/surface_height.ts

import type { f64 } from "@xpute/core/kit/word.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { get_ele_at_local } from "./terrain/elevation.ts";
import { local_world_to_canonical } from "./frame/coord.ts";
import { get_objects_near } from "./objects/store.ts";
import { object_aabb } from "./objects/geometry.ts";

// Half a cell is a probe depth that's always safely *inside* whichever
// cell a surface sits at the bottom of, never exactly on the boundary
// between it and the empty space below. Probing exactly at a surface's
// own Z (this function's first, since-fixed version did exactly that)
// puts floor_div right on a cell edge, where float noise from the PVF3
// round-trip can tip the resulting cell index either way — observed
// directly as an object 2+ cells up a stack reading as if the whole stack
// were only 1 cell tall, every time, regardless of the stack's real
// height.
const CELL_PROBE_DEPTH: f64 = U_PER_CELL / 2;

// get_surface_z_at_local calls get_objects_near (a fixed ±1-cell
// neighborhood, built for nearby collision/hit-testing, not an arbitrary-
// range scan) once per cell stepped through, both hunting upward for a
// snap-up target and climbing a contiguous stack — this bound is just a
// runaway guard, comfortably above any real structure a person would
// place by hand.
const MAX_PROBES = 64;

// Two Z values within this of each other count as "flush" — guards the
// climb loop against re-finding the object that just set `surface` on the
// next pass, and the "is this actually resting there" / "is this at or
// past the hint" checks below, against float noise from the same PVF3
// round-trip conversions object_local_corner's own OVERLAP_EPS guards
// against elsewhere.
const SAME_SURFACE_EPS: f64 = 1e-6;

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
  function objects_at(probe_z: f64): { min: V3; max: V3 }[] {
    const probe: V3 = [local_x, local_y, probe_z];
    const corner = local_world_to_canonical({ i: 0n, f: 0n }, probe, map_origin);
    return get_objects_near(corner, 0)
      .map((obj) => object_aabb(obj, map_origin))
      .filter((aabb) => local_x >= aabb.min[0] && local_x < aabb.max[0] && local_y >= aabb.min[1] && local_y < aabb.max[1]);
  }

  // Snap-up search: step upward one cell at a time from the hint (each
  // probe's own ±1-cell neighborhood plus a full-cell stride keeps the
  // scan gap-free) for the lowest qualifying top at or above it.
  const search_from = Math.max(hint_z, terrain_ele);
  let landing: f64 | null = null;
  for (let i = 0; i < MAX_PROBES && landing === null; i++) {
    for (const aabb of objects_at(search_from + i * U_PER_CELL)) {
      if (aabb.max[2] < hint_z - SAME_SURFACE_EPS) continue;
      if (landing === null || aabb.max[2] < landing) landing = aabb.max[2];
    }
  }

  let surface = landing ?? terrain_ele;

  // Climb any further objects resting flush on top of the landing spot —
  // the whole point is standing on the *true* top of whatever contiguous
  // structure is under the character, not wherever the snap-up search (or
  // the ground, in the fallback case) happened to land.
  for (let i = 0; i < MAX_PROBES; i++) {
    let top: f64 | null = null;
    for (const aabb of objects_at(surface + CELL_PROBE_DEPTH)) {
      if (aabb.min[2] > surface + SAME_SURFACE_EPS) continue;
      if (aabb.max[2] <= surface + SAME_SURFACE_EPS) continue;
      if (top === null || aabb.max[2] > top) top = aabb.max[2];
    }
    if (top === null) break;
    surface = top;
  }

  return surface;
}
