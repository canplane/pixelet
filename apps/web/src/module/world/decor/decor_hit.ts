// @pxl/web/module/world/decor/decor_hit.ts

/**
 * Which user-placed decor entry the pointer's ray strikes, by the module's
 * hit test (crates/pxl-core/src/raycast.rs's hit_decor): the ray against
 * the triangles of every placed instance the on-screen chunks show, from
 * the same blocks the renderer draws. What comes back is the chunk it
 * stands in and the entry's band and packed position in that column, which
 * the object store turns into the entry's id.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { Vec3 } from "@xpute/core/kit/mat4.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { CORE_PXL } from "@/render/layer/pipeline/core_pool.ts";
import { chunk_nodes } from "@/module/world/chunk_node.ts";
import { entry_id_of } from "@/module/world/object/store.ts";

// A decor mesh sitting on the ground it also intersects (any tree's own
// trunk-base, by construction) has its own hit distance land almost exactly
// AT max_dist, not comfortably inside it — the two distances come from two
// independent algorithms (a triangle test against the model here, the
// voxel walk's slab test for max_dist), so they only ever agree up to float
// noise. A strict reject flipped on essentially every frame right at that
// boundary as a real mouse's sub-pixel jitter walked the disagreement across
// zero, which read as the hover ghost flickering while the cursor sat still
// (2026-09-05). 1e-3 units is small enough never to let a decor mesh that is
// genuinely occluded by terrain win, and larger than the round-off.
const SAME_SURFACE_EPS: f64 = 1e-3;

/**
 * The id of the placed decor entry the ray strikes within `max_dist`, or
 * null. Procedural instances are passed over by the module, so a hit on one
 * falls through to whatever is next along the ray. The ray is in the root
 * frame, relative to `map_origin`, as the voxel raycast takes it.
 */
export function raycast_decor(origin: Vec3, direction: Vec3, map_origin: IVec, max_dist: f64): string | null {
  const p = CORE_PXL;
  if (!p) return null;
  const struck = p.pxl_raycast_decor(
    ivec.to_real(ivec.X(map_origin)),
    ivec.to_real(ivec.Y(map_origin)),
    origin.x,
    origin.y,
    origin.z,
    direction.x,
    direction.y,
    direction.z,
    max_dist + SAME_SURFACE_EPS,
  );
  if (!struck) return null;
  const [chunk, band, pos] = new Float64Array(p.memory.buffer, p.pxl_raycast_decor_at(), 3);
  const node = chunk_nodes.get(chunk);
  if (!node) return null;
  return entry_id_of(node.object_key, band, pos);
}
