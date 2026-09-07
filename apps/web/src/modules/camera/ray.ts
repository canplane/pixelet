// @pxl/web/modules/camera/ray.ts

/**
 * Single raycast entry point for the whole world — terrain (any face: top
 * or a cliff/wall side) and placed objects (any size, including sub-cell)
 * alike, since both are painted into the exact same sectioned per-column voxel
 * field (render/layer/pipeline/volume.ts) chunk_stage.tsx renders from. This
 * used to be two+ separate systems (a 2D heightfield march for terrain
 * tops, a second DDA for terrain walls, and a third real-AABB pass over the
 * raw PlacedObject[] for objects) that could each find a different answer
 * for what should be the same physical surface; now there's one DDA that
 * walks composite_chunk()'s own voxel-derived boxes (via
 * render/layer/pipeline/voxel_scan.ts's merge_column_runs/run_to_box — the SAME functions
 * chunk_stage.tsx meshes with) and returns whichever box the ray actually
 * struck first, regardless of whether it came from terrain or an object.
 *
 * Walks every chunk the ray's own XY projection actually crosses within
 * max_dist (ray_hit_world's own outer chunk-DDA, ray_hit_chunk's own inner
 * per-cell DDA within each) — not just whichever one chunk happens to
 * contain some reference point. composite_chunk isn't free, but it's
 * memoized per chunk (compositor.ts's own cache) against the handful of
 * things that can actually invalidate it, so re-visiting the same chunk
 * across repeated hover ticks (the common case — the cursor doesn't teleport
 * between frames) doesn't pay full recompute cost each time, and max_dist
 * itself (ray_pick's own INTERACT_RADIUS_U*1.5) keeps a genuinely fresh area
 * to a small local set of chunks.
 */

import * as THREE from "three";

import type { f64, u32 } from "@xpute/core/kit/word.ts";

import { type IVec, ivec, projection, type PVF3, pvf3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L2_IDX_IN_L3, L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { CHUNK_CELL_SPAN, U_PER_CELL } from "@pxl/shared/bake/constants.ts";
import { with_ele } from "@/modules/world/terrain/elevation.ts";
import { get_local_world_origin, local_world_to_canonical } from "../world/frame/coord.ts";
import { aabb_overlap, object_aabb } from "@/modules/world/objects/geometry.ts";
import { get_objects_near, type OccupiedEntry } from "@/modules/world/objects/store.ts";

import { with_raycast_composite } from "@/render/layer/pipeline/compositor.ts";
import { chunk_origin_of } from "@/render/layer/objects/adapter.ts";
import { CHUNK_STAGE_LAYERS } from "@/modules/world/stack.ts";
import { MaterialClass } from "@/render/layer/pipeline/volume.ts";
import { ray_hit_chunk, type WorldHit } from "@/render/layer/pipeline/chunk_raycast.ts";
import type { ChunkKey } from "@/render/layer/pipeline/contract.ts";

const $origin = new THREE.Vector3();
const $dir = new THREE.Vector3();
const $m = new THREE.Matrix4();

export type { WorldHit } from "@/render/layer/pipeline/chunk_raycast.ts";

const INTERACT_RADIUS_CELL: f64 = 32 / U_PER_CELL - .707;
const INTERACT_RADIUS_U: f64 = INTERACT_RADIUS_CELL * U_PER_CELL;
const INTERACT_RADIUS2_U: f64 = INTERACT_RADIUS_U * INTERACT_RADIUS_U;

export function to_local_world_ray(out: THREE.Ray, ray: THREE.Ray, world_frame: THREE.Group | null, local_world_frame: THREE.Group | null): THREE.Ray {
  $m.identity();

  if (local_world_frame) {
    $m.copy(local_world_frame.matrixWorld);
  } else if (world_frame) {
    $m.copy(world_frame.matrixWorld);
  }

  $m.invert();

  $origin.copy(ray.origin).applyMatrix4($m);
  $dir.copy(ray.direction).transformDirection($m).normalize();
  return out.copy(ray).set($origin, $dir);
}

export function in_interact_radius(a: PVF3, b: PVF3): boolean {
  const dx: f64 = pvf3.to_real(pvf3.X(a)) - pvf3.to_real(pvf3.X(b));
  const dy: f64 = pvf3.to_real(pvf3.Y(a)) - pvf3.to_real(pvf3.Y(b));
  return dx * dx + dy * dy <= INTERACT_RADIUS2_U;
}

const CHUNK_W: f64 = CHUNK_CELL_SPAN * U_PER_CELL;

/**
 * Single raycast entry point's real body — an outer 2D DDA over CHUNK_W-
 * sized cells (same shape as ray_hit_chunk's own per-cell DDA, one grid
 * size coarser), walking every chunk the ray's XY projection crosses
 * within max_dist and testing each via ray_hit_chunk. Non-decreasing
 * entry-t order, so the first chunk reporting a hit holds the nearest one.
 *
 * Chunk keys come from the ray's own trajectory, not map_center — an
 * earlier version tested only the one chunk containing map_center (the pan
 * target), which silently found nothing the moment the cursor wasn't near
 * screen center, since max_dist can reach into a neighboring chunk that
 * was never even composited.
 */
function ray_hit_world(ray: THREE.Ray, map_origin: IVec, max_dist: f64): WorldHit | null {
  const ox: f64 = ray.origin.x;
  const oy: f64 = ray.origin.y;
  const dx: f64 = ray.direction.x;
  const dy: f64 = ray.direction.y;

  // grid_cx/grid_cy are chunk-grid indices in ray-local (map_origin-
  // relative) space — floor(local_x / CHUNK_W), the chunk-level analogue of
  // ray_hit_chunk's own cell_x/cell_y.
  function test_chunk(grid_cx: f64, grid_cy: f64): WorldHit | null {
    // A point comfortably inside this chunk-grid cell's own footprint
    // (its center) — which real chunk that resolves to is what matters,
    // not this exact sub-position within it.
    const sample_x: f64 = (grid_cx + 0.5) * CHUNK_W;
    const sample_y: f64 = (grid_cy + 0.5) * CHUNK_W;
    const abs: PVF3 = local_world_to_canonical({ i: 0n, f: 0n }, [sample_x, sample_y, 0], map_origin);
    const page_idx: IVec = L3_IDX(abs.i);
    const page_off: IVec = L2_IDX_IN_L3(abs.i);
    const key: ChunkKey = { page_idx, page_off };

    return with_raycast_composite(CHUNK_STAGE_LAYERS, key, (volume) => {
      if (!volume) return null;
      const chunk_origin: IVec = chunk_origin_of(page_idx, page_off);
      const delta: IVec = ivec.sub(chunk_origin, map_origin);
      const chunk_ox: f64 = ivec.to_real(ivec.X(delta));
      const chunk_oy: f64 = ivec.to_real(ivec.Y(delta));
      return ray_hit_chunk(ray, volume, chunk_ox, chunk_oy, max_dist);
    });
  }

  // A ray that never crosses a chunk boundary in XY (looking straight
  // down/up) can only ever be over the one chunk it started above.
  if (Math.abs(dx) < 1e-9 && Math.abs(dy) < 1e-9) {
    return test_chunk(Math.floor(ox / CHUNK_W), Math.floor(oy / CHUNK_W));
  }

  let grid_cx: f64 = Math.floor(ox / CHUNK_W);
  let grid_cy: f64 = Math.floor(oy / CHUNK_W);

  const step_x: f64 = dx > 0 ? 1 : -1;
  const step_y: f64 = dy > 0 ? 1 : -1;

  const t_delta_x: f64 = Math.abs(dx) < 1e-9 ? Infinity : CHUNK_W / Math.abs(dx);
  const t_delta_y: f64 = Math.abs(dy) < 1e-9 ? Infinity : CHUNK_W / Math.abs(dy);

  let t_max_x: f64 = Math.abs(dx) < 1e-9 ? Infinity : ((dx > 0 ? grid_cx + 1 : grid_cx) * CHUNK_W - ox) / dx;
  let t_max_y: f64 = Math.abs(dy) < 1e-9 ? Infinity : ((dy > 0 ? grid_cy + 1 : grid_cy) * CHUNK_W - oy) / dy;

  // max_dist is typically well under one CHUNK_W (INTERACT_RADIUS_U*1.5 —
  // see this file's own doc comment), so this is usually just 1-3 chunks,
  // never unbounded regardless of how far along the ray max_dist reaches.
  const max_chunks: u32 = Math.ceil(max_dist / CHUNK_W) + 2;

  for (let i = 0; i < max_chunks; i++) {
    const found = test_chunk(grid_cx, grid_cy);
    if (found) return found;

    // Every remaining chunk boundary is already farther than max_dist —
    // nothing left within range to find, same early-exit reasoning
    // ray_hit_chunk's own per-cell DDA gets for free from its bounded
    // max_cells (a chunk-level step is too coarse for that same bound to
    // fall out of the loop count alone, so this checks distance directly).
    if (Math.min(t_max_x, t_max_y) > max_dist) break;

    if (t_max_x < t_max_y) {
      grid_cx += step_x;
      t_max_x += t_delta_x;
    } else {
      grid_cy += step_y;
      t_max_y += t_delta_y;
    }
  }

  return null;
}

/** The struck top terrain cell's own (min-x, min-y, min-z) CORNER — for the
 * pointer tool's hover cursor over plain terrain OR a wall (both give the
 * same "which cell" answer via box_min's own XY, since a wall hit's box is
 * still that wall's OWN cell, not a neighbor's). X/Y come straight from
 * box_min; Z is the real terrain surface (with_ele) stepped DOWN one cell,
 * so the anchor is the struck voxel's floor, not its top face — the same
 * min-corner convention world_struck_corner / world_adjacent_corner already
 * use, which lets the ghost (markers/hover_cursor.tsx) and the recenter
 * (camera/input.tsx's anchor_center) both apply a symmetric + half a cell on
 * all three axes. Objects don't go through this — see resolve_hit_object,
 * which hands back the real PlacedObject's own i/f directly instead. */
export function world_hover_pvf3(hit: WorldHit, map_origin: IVec): PVF3 | null {
  // -U_PER_CELL, not 0: with_ele adds the real surface height onto this Z, so
  // starting one cell low lands the result on the struck top voxel's floor
  // (min-z corner) instead of its top face — see this function's doc comment.
  const p: PVF3 = projection.project({ i: 0n, f: 0n }, hit.box_min[0], hit.box_min[1], -U_PER_CELL);
  p.i = ivec.add(p.i, map_origin);
  return with_ele(p, p.i, p.f);
}

/**
 * The corner (min-x/y/z) a new `new_size` cube would occupy flush against
 * whatever `hit` struck — terrain top, terrain wall, or an existing object,
 * all the same formula now that every hit carries its own real box_min/
 * box_max rather than a size/corner pulled from a specific PlacedObject.
 * Per axis: the axis the hit's face_normal points along snaps flush against
 * that face; the other two axes either sub-tile within the struck box's own
 * footprint (new_size smaller than that box's own extent there — e.g.
 * placing a sub-cell cube on a full terrain cell, or on a larger existing
 * object's face) or quantize to the new cube's own absolute grid (new_size
 * equal-or-larger — classic Minecraft-style stacking, no anchoring on the
 * struck box's own exact position).
 */
// Tolerance for "same nominal size"/"integer-aligned" comparisons below —
// bigger than the sub_cell 1/256-grid round trip's own worst-case error
// (±U_PER_CELL/512, voxel.ts's pack/unpack pair), smaller
// than the gap between any two OBJECT_SIZES tiers (smallest U_PER_CELL/8).
const SAME_EXTENT_EPS: f64 = U_PER_CELL / 128;

/** Smallest multiple of `size` that is >= v (epsilon-fuzzed so a v already
 * ON the grid stays put instead of bumping to the next line on float
 * noise) — the sparse_tree octree (objects/store.ts) only ever stores a
 * `size`-tier object at one of these lines, on EVERY axis, so this is the
 * one honest "where can a new_size box's near edge legally start" answer;
 * anything else just gets silently re-snapped by write() itself later,
 * which is exactly what caused the overlap bug this fixes (see
 * world_adjacent_corner's own doc comment). */
function grid_ceil(v: f64, size: f64): f64 {
  return Math.ceil((v - SAME_EXTENT_EPS) / size) * size;
}

/** Largest multiple of `size` that is <= v — grid_ceil's mirror. */
function grid_floor(v: f64, size: f64): f64 {
  return Math.floor((v + SAME_EXTENT_EPS) / size) * size;
}

/**
 * The corner (min-x/y/z) a new `new_size` cube would occupy flush against
 * whatever `hit` struck — ONE rule, applied identically to all three axes,
 * not a different formula for "the struck face's own axis" vs "the other
 * two": every axis lands on new_size's own sparse_tree-canonical grid
 * (objects/store.ts's place() ultimately floors/masks to exactly this same
 * grid via its own bit-shift addressing — see grid_ceil/grid_floor above),
 * and which of the two grid-snap directions applies is purely a function of
 * that axis's own face_normal component:
 * - n > 0 (this axis IS the struck face's outward normal, positive side):
 *   the new box must start at-or-beyond the struck face — grid_ceil(box_max).
 * - n < 0 (outward normal, negative side): the new box must end at-or-before
 *   the struck face — grid_floor(box_min) - new_size.
 * - n === 0 (this axis is untouched by the struck face — the new box is
 *   free to land anywhere along it): whichever new_size-grid cell hit_point
 *   itself falls into — grid_floor(hit_point). Automatically stays within
 *   the struck box's own footprint when new_size <= the struck box's own
 *   extent on this axis (both box_min and new_size's grid nest inside the
 *   struck box's own coarser grid — no separate clamp needed), and
 *   automatically resolves to the struck box's own containing cell when
 *   new_size is bigger (hit_point is inside the struck box, so the coarser
 *   cell containing it is the same one the struck box itself sits in).
 *
 * A struck face can put its nonzero n on ANY axis (a side wall's normal is
 * X or Y, not just a top hit's Z), so "the struck face's axis" was never
 * actually "X/Y" or "Z" specifically — treating it as a special case
 * per-axis (this function's old shape) just meant the SAME axis got two
 * different formulas depending on which one you clicked, silently
 * overlapping when the struck box's own extent wasn't already new_size's
 * own multiple. One rule, chosen per axis by that axis's own face_normal
 * sign, is both simpler and the only version that can't disagree with
 * itself.
 *
 * `precise_box`: use the caller's exact, known PlacedObject AABB
 * (object_aabb, geometry.ts) instead of `hit.box_min/box_max` when
 * available. hit's own box for a sub-cell object comes back through
 * voxel.ts's u8-packed sub_cell payload — accurate to only ~1/510 of a
 * cell — so stacking off it compounded that error stack-to-stack, visibly
 * (if inconsistently) drifting. Terrain/unknown hits have no more precise
 * source, so they keep using hit's own box (plain float64 precision,
 * nowhere near this error scale).
 *
 * Also snaps a whole-cell(-or-larger) result to a real cell boundary
 * (below) — objects/store.ts's place() can't store that tier at a
 * fractional height (sparse_tree.fill() claims a whole integer cell, no
 * sub-cell payload), so this is the one place to fix it: both the hover
 * preview and the real placement read this exact same PVF3, so snapping
 * here (not separately downstream in place()) is what keeps the preview,
 * the overlap/blocked check, and the actual write all agreeing on the same
 * target cell.
 */
export function world_adjacent_corner(hit: WorldHit, new_size: f64, map_origin: IVec, precise_box?: { min: V3; max: V3 }): PVF3 {
  const { face_normal, hit_point } = hit;
  const box_min = precise_box?.min ?? hit.box_min;
  const box_max = precise_box?.max ?? hit.box_max;
  const out: V3 = [0, 0, 0];

  for (const axis of [0, 1, 2] as const) {
    const n = face_normal[axis];
    if (n > 0) {
      out[axis] = grid_ceil(box_max[axis], new_size);
    } else if (n < 0) {
      out[axis] = grid_floor(box_min[axis], new_size) - new_size;
    } else {
      out[axis] = grid_floor(hit_point[axis], new_size);
    }
  }

  // Whole-cell(-or-larger) snap to a real cell boundary — see this
  // function's own doc comment.
  if (new_size >= U_PER_CELL - SAME_EXTENT_EPS) {
    for (const axis of [0, 1, 2] as const) {
      const cells = out[axis] / U_PER_CELL;
      const frac = cells - Math.floor(cells);
      if (frac < SAME_EXTENT_EPS || frac > 1 - SAME_EXTENT_EPS) continue; // already (near-)integer.
      out[axis] = Math.ceil(cells) * U_PER_CELL;
    }
  }

  return local_world_to_canonical({ i: 0n, f: 0n }, out, map_origin);
}

/**
 * The single real `size`-tall voxel `hit` actually struck, exactly as
 * raycast against the live rendered voxel field — eraser's own targeting
 * for its "nothing placed here, dig into the terrain itself" case.
 * Deliberately NOT just hit.box_min/box_max directly — those bound whichever
 * merged run the mesher/raycaster happened to combine same-material voxels
 * into for efficiency (voxel_scan.ts's merge_column_runs). A flat,
 * uniformly-colored terrain column routinely merges dozens of cells deep
 * into ONE box, so box_min there can sit far below the real surface, nowhere
 * near what was actually visually struck — an earlier version of this
 * function used box_min as-is, and both the hover ghost and the real write
 * landed at that buried address instead: invisible, and no-op-looking, even
 * though the write genuinely happened.
 *
 * Same per-axis shape as world_adjacent_corner (this file, one rule per
 * axis, direction chosen by that axis's own face_normal sign) but the
 * OPPOSITE choice on the axis with a nonzero normal: world_adjacent_corner
 * steps OUTSIDE the struck box on that axis (flush-against, for placing a
 * NEW box beside it); this stays INSIDE it, landing on the exact `size`-tall
 * slice the struck face itself bounds — one cell below the boundary for a
 * n>0 face (a top, or a wall's +axis side), one cell at-and-above it for a
 * n<0 face (an underside, or a wall's -axis side) — regardless of how many
 * more identical voxels the run actually merged beneath/behind it. The one
 * axis with no face_normal component (both, for an edge-on hit) instead
 * uses hit_point itself, same as world_adjacent_corner's own n===0 case:
 * whichever `size`-grid cell the ray actually landed in, since only
 * hit_point — not box_min/box_max — is guaranteed to fall inside the ONE
 * real cell that was struck rather than the merged run's whole footprint.
 */
export function world_struck_corner(hit: WorldHit, size: f64, map_origin: IVec): PVF3 {
  const { face_normal, hit_point, box_min, box_max } = hit;
  const out: V3 = [0, 0, 0];

  for (const axis of [0, 1, 2] as const) {
    const n = face_normal[axis];
    if (n > 0) {
      out[axis] = grid_ceil(box_max[axis], size) - size;
    } else if (n < 0) {
      out[axis] = grid_floor(box_min[axis], size);
    } else {
      out[axis] = grid_floor(hit_point[axis], size);
    }
  }

  return local_world_to_canonical({ i: 0n, f: 0n }, out, map_origin);
}

/** Which solid placed object `hit` actually struck, when hit.material_class
 * is MaterialClass.OBJECT — whichever placed object's own real AABB
 * (object_aabb — full precision, including sub-cell) overlaps the hit box.
 * The sectioned voxel field itself carries no object identity
 * (chunk_stage.tsx never needs it, only color/material), so this is a small
 * secondary lookup, not the raycast itself. Decor is never resolved here —
 * it never paints a voxel of its own (render/layer/objects/adapter.ts's
 * paint() skips "decor" entries) and has its own real-mesh hit-test now
 * that it bakes into a real THREE.Mesh instead (decor_hit.ts's
 * raycast_decor, called directly from camera/input.tsx alongside this
 * function, since it needs the live THREE.Raycaster this module never
 * touches). Linear scan over the same small objects list every other
 * object operation already walks (see objects/adapter.ts's own "fine at
 * current placement counts" doc comment) — not the raycast itself, just
 * resolving an already-found hit back to its source. */
export function resolve_hit_object(hit: WorldHit, map_origin: IVec): OccupiedEntry | null {
  if (hit.material_class !== MaterialClass.OBJECT) return null;
  const hit_aabb = { min: hit.box_min, max: hit.box_max };

  // Only checks objects near the hit point (store.ts's own get_objects_near
  // — the containing cell plus its 26 neighbors), not every placed object
  // in the world — same reasoning objects/collision.ts's own
  // find_overlapping_object already applies.
  const hit_point_abs: PVF3 = local_world_to_canonical({ i: 0n, f: 0n }, hit.hit_point, map_origin);
  const nearby = get_objects_near(hit_point_abs, U_PER_CELL);

  // Nearest-to-hit_point match, not first-in-array-order — real objects
  // never truly overlap (placement-time collision checks forbid it), so
  // this should normally degenerate to "the only match" regardless of
  // order, but aabb_overlap's OVERLAP_EPS tolerance (geometry.ts) means two
  // objects flush against each other can both register as touching the
  // same hit_aabb right at their shared boundary — picking whichever is
  // actually closest to where the ray landed resolves that correctly
  // instead of silently depending on iteration order.
  let best: OccupiedEntry | null = null;
  let best_dist2 = Infinity;
  for (const o of nearby) {
    const aabb = object_aabb(o, map_origin);
    if (!aabb_overlap(aabb, hit_aabb)) continue;
    const cx = (aabb.min[0] + aabb.max[0]) * 0.5;
    const cy = (aabb.min[1] + aabb.max[1]) * 0.5;
    const cz = (aabb.min[2] + aabb.max[2]) * 0.5;
    const dx = cx - hit.hit_point[0], dy = cy - hit.hit_point[1], dz = cz - hit.hit_point[2];
    const dist2 = dx * dx + dy * dy + dz * dz;
    if (dist2 < best_dist2) {
      best = o;
      best_dist2 = dist2;
    }
  }
  return best;
}

export type PickResult = {
  /** Never null — ray_pick itself returns null (not a PickResult with a null hit) when nothing was struck. */
  hit: WorldHit;
};

/** Single raycast entry point — see this file's own doc comment. */
export function ray_pick(
  out: THREE.Ray,
  ray: THREE.Ray,
  world_frame: THREE.Group | null,
  local_world_frame: THREE.Group | null,
  map_center: PVF3 | null,
): PickResult | null {
  if (!map_center) return null;
  const map_origin = get_local_world_origin(map_center);

  const local_ray = to_local_world_ray(out, ray, world_frame, local_world_frame);
  const hit = ray_hit_world(local_ray, map_origin, INTERACT_RADIUS_U * 1.5);
  if (!hit) return null;

  const probe: PVF3 = local_world_to_canonical({ i: 0n, f: 0n }, hit.hit_point, map_origin);
  if (!in_interact_radius(probe, map_center)) return null;

  return { hit };
}
