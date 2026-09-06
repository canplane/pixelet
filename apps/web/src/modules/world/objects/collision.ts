// @pxl/web/modules/world/objects/collision.ts

import type { f64 } from "@xpute/core/kit/word.ts";
import type { IVec, PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { aabb_overlap, corner_aabb, object_aabb } from "./geometry.ts";
import { get_objects_near, type OccupiedEntry } from "./store.ts";
import { DECOR_SIZE } from "./size.ts";

/**
 * Whether a candidate box (corner + size) overlaps any existing placed
 * object — real per-axis AABB overlap, not the old same-size grid-key
 * equality check (objects/columns.ts, removed once real object raycasting
 * — camera/ray.ts's ray_pick — made it redundant). Catches cross-size
 * overlap too, which the grid-key check never could.
 *
 * Only checks objects near `corner` (store.ts's own get_objects_near — the
 * candidate's own cell plus its 26 neighbors), not every placed object in
 * the world — a real object count can be arbitrarily large, and a
 * placement can only ever conflict with something within one cell's own
 * width of it regardless.
 */
export function find_overlapping_object(corner: PVF3, size: f64, map_origin: IVec): OccupiedEntry | null {
  const candidate = corner_aabb(corner, size, map_origin);
  const nearby = get_objects_near(corner, size);
  return nearby.find((o) => aabb_overlap(candidate, object_aabb(o, map_origin))) ?? null;
}

/**
 * Would placing this tool's own candidate at `corner` right now actually
 * land somewhere already occupied — the one "object size vs decor's own
 * fixed DECOR_SIZE" dispatch both the real placement commit
 * (camera/input.tsx's handle_tap) and its own hover preview
 * (markers/hover_cursor.tsx's `blocked`) need, each used to independently
 * reimplement this exact two-branch size pick before landing here.
 */
export function is_placement_blocked(tool: "object" | "decor", corner: PVF3, object_size: f64, map_origin: IVec): boolean {
  const size = tool === "decor" ? DECOR_SIZE : object_size;
  return find_overlapping_object(corner, size, map_origin) !== null;
}
