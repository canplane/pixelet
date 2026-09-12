// @pxl/web/modules/world/decor/decor_hit.ts

/**
 * Real mesh-based hit-testing for decor — replaces the exact-cell-lookup
 * workaround camera/ray.ts used to carry ("decor never paints a voxel, so
 * resolve whatever cell the ray struck instead"), from back when decor had no
 * addressable per-instance identity at all. A real THREE.Raycaster pass hits
 * the model's own silhouette rather than its cell.
 *
 * There is no registry here any more. field.ts holds one instanced mesh per
 * variant and hands back an `instanceId` for a hit, which is a direct index
 * into what it drew — where this used to keep its own parallel record of
 * every mounted mesh and divide a `faceIndex` by a faces-per-instance stride
 * to recover which instance a triangle belonged to. That arithmetic was only
 * ever a way to ask an instance question of a mesh that had baked its
 * instances away.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type * as THREE from "three";

import { decor_field } from "./field.ts";

// A decor mesh sitting on the ground it also intersects (any tree's own
// trunk-base, by construction) has its own hit distance land almost exactly
// AT max_dist, not comfortably inside it — the two distances are computed by
// two entirely independent algorithms (THREE's own Möller-Trumbore triangle
// test against decor's own geometry here, chunk_raycast.ts's own closed-form
// ray/AABB-slab test against a voxel-derived box for max_dist itself), not
// two evaluations of the same formula, so they only ever agree up to float
// noise, never exactly. A strict `hit.distance > max_dist` reject flips on
// essentially every frame right at that boundary — a real mouse's own
// sub-pixel sensor jitter (no automation, no camera motion needed) perturbs
// the ray just enough each frame to walk the disagreement across zero in
// either direction, which read as the hover ghost itself flickering on and
// off while the cursor sat still (reported 2026-09-05, right after
// hover_cursor.tsx's own real-silhouette ghost made a hovered
// hovered_object_id toggle newly visible as a visual flicker rather than a
// silent no-op). 1e-3 world units is small enough to never let a decor mesh
// that's genuinely occluded by terrain win (that gap is always at least a
// real fraction of a cell, orders of magnitude bigger), comfortably larger
// than pure float64 round-off between the two algorithms.
const SAME_SURFACE_EPS: f64 = 1e-3;

/**
 * Which user-placed decor entry's own id (objects/store.ts leaf id) the
 * raycaster's current ray actually struck within max_dist, or null if nothing
 * did — INCLUDING when the closest real geometry hit belongs to a procedural
 * (scatter.ts-derived, never-stored, id-less) placement: procedural decor has
 * always been transparent to hit-testing, so a hit there is skipped in favour
 * of whatever's next along the ray, not treated as "found, but unselectable".
 *
 * `max_dist` bounds this to distances the caller's own already-found
 * terrain/object hit doesn't already beat — decor sitting on solid ground can
 * only ever be the right answer when it's the CLOSER thing along the ray.
 */
export function raycast_decor(raycaster: THREE.Raycaster, max_dist: f64): string | null {
  const meshes = decor_field.hit_meshes();
  if (meshes.length === 0) return null;
  const hits = raycaster.intersectObjects(meshes, false);

  for (const hit of hits) {
    if (hit.distance > max_dist + SAME_SURFACE_EPS) break; // sorted nearest-first
    if (hit.instanceId === undefined) continue;
    const id = decor_field.entry_at(hit.object, hit.instanceId);
    if (id) return id;
  }
  return null;
}
