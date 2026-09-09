// @pxl/web/modules/world/objects/geometry.ts

import type { f64 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import { U_PER_CELL } from "@pxl/shared/bake/constants.ts";

import { local_world_to_canonical } from "@/modules/world/frame/coord.ts";
import type { OccupiedEntry } from "./store.ts";

// Half a cell on one axis, as a packed fixed-point scalar.
const HALF_CELL_FP = pvf3.from_real(U_PER_CELL / 2);

/**
 * The center of the cell a hover anchor (`i`/`f`) points at — the point that
 * anchor actually *represents* to the player: where the camera recenters when
 * you tap, which is exactly the spot the hover-cursor ghost box is drawn
 * centered on. "Tap the highlighted box, the camera goes to that box."
 *
 * `+ half` on every axis, symmetric — a hover anchor is a cell's (min-x,
 * min-y, min-z) corner on all three (camera/ray.ts: X/Y straight from
 * box_min, Z the surface stepped down one cell to the struck voxel's floor),
 * so an anchor at (x,y,z) occupies (x,y,z)..(x+1,y+1,z+1) and its middle is
 * (x+.5, y+.5, z+.5) — v3.ts's own span3/center3 convention, Minecraft's
 * block model. object_center below is the placed-cube counterpart: the exact
 * same shape, since a cube's anchor is a true min corner too.
 *
 * decor/decor_layer.ts centers user-placed decor the same way on X/Y but
 * keeps Z on the slot (a decor model's origin sits at its base, not its
 * middle) — the X/Y half of this, not the whole thing.
 */
export function anchor_center(anchor: PVF3): PVF3 {
  return pvf3.gather(
    { i: 0n, f: 0n },
    pvf3.X(anchor) + HALF_CELL_FP,
    pvf3.Y(anchor) + HALF_CELL_FP,
    pvf3.Z(anchor) + HALF_CELL_FP,
  );
}

/**
 * obj.i/f is this object's corner (min-x, min-y, min-z) — the same
 * convention render/layer/pipeline/chunk_mesher.ts and markers/hover_cursor.tsx render from
 * (both add +size/2 on every axis to get the visual center). Returned in
 * map_origin-relative real units, the same low-magnitude space AnchorNode's
 * own local_pos and camera/ray.ts's terrain raycast already operate in.
 */
export function object_local_corner(obj: Pick<OccupiedEntry, "i" | "f">, map_origin: IVec): V3 {
  const local: PVF3 = { i: ivec.sub(obj.i, map_origin), f: obj.f };
  return [pvf3.to_real(pvf3.X(local)), pvf3.to_real(pvf3.Y(local)), pvf3.to_real(pvf3.Z(local))];
}

export type AABB = { min: V3; max: V3 };

export function object_aabb(obj: OccupiedEntry, map_origin: IVec): AABB {
  return corner_aabb(obj, obj.size, map_origin);
}

/** Same box shape object_aabb produces, for a candidate corner+size that isn't (yet) a stored OccupiedEntry — e.g. a placement/overlap check before committing. */
export function corner_aabb(corner: Pick<OccupiedEntry, "i" | "f">, size: f64, map_origin: IVec): AABB {
  const min = object_local_corner(corner, map_origin);
  const max: V3 = [min[0] + size, min[1] + size, min[2] + size];
  return { min, max };
}

// Two AABBs must overlap by MORE than this to count — a flush-touching pair
// (one box's own face exactly against another's, e.g. stacking) is supposed
// to read as zero overlap, but both AABBs here are built by re-deriving
// real coordinates from PVF3 (object_local_corner's own to_real calls) at
// each of possibly several chained placements (world_adjacent_corner
// computing a new corner directly off an existing object's own AABB, stack
// after stack) — a strict `<` with no tolerance treats even a single ULP of
// drift either direction as a real verdict, silently flipping a legitimate
// flush placement to "blocked" (or the reverse: a real, tiny overlap
// passing as clear). Comfortably bigger than realistic float64 noise over a
// handful of chained conversions, comfortably smaller than the smallest
// real gap two placements could ever legitimately have (any OBJECT_SIZES
// tier, U_PER_CELL/8 at minimum).
const OVERLAP_EPS: f64 = 1e-6;

export function aabb_overlap(a: AABB, b: AABB): boolean {
  return a.min[0] < b.max[0] - OVERLAP_EPS && b.min[0] < a.max[0] - OVERLAP_EPS &&
    a.min[1] < b.max[1] - OVERLAP_EPS && b.min[1] < a.max[1] - OVERLAP_EPS &&
    a.min[2] < b.max[2] - OVERLAP_EPS && b.min[2] < a.max[2] - OVERLAP_EPS;
}

/** This object's true center (corner + size/2 on every axis), as an absolute PVF3 — e.g. for recentering the map on an object rather than its corner. */
export function object_center(obj: OccupiedEntry, map_origin: IVec): PVF3 {
  const corner = object_local_corner(obj, map_origin);
  const half = obj.size / 2;
  const center: V3 = [corner[0] + half, corner[1] + half, corner[2] + half];
  return local_world_to_canonical({ i: 0n, f: 0n }, center, map_origin);
}
