// @pxl/web/module/world/decor/decor_hit.ts

/**
 * Which user-placed decor entry the pointer's ray strikes, by the module's
 * hit test (crates/pxl/core/src/world/raycast.rs's hit_decor): the ray against
 * the triangles of every placed instance the on-screen chunks show, from
 * the same blocks the renderer draws. What comes back is the chunk it
 * stands in and the entry's band and packed position in that column, which
 * the object store turns into the entry's id.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { Vec3 } from "@xpute/core/kit/mat4.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";

import { CORE_BUILD } from "@/wasm/core_build.ts";
import { ray_packet } from "@/module/world/raycast.ts";
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
  const hit = CORE_BUILD.call(Cmd.DECOR_READ, ray_packet(map_origin, origin, direction, max_dist + SAME_SURFACE_EPS), (struck, reply) => {
    if (struck !== 1 || reply === null) return null;
    return { chunk: reply.at(0).get<number>(), band: reply.at(1).get<number>(), pos: reply.at(2).get<number>() };
  });
  if (hit === null) return null;
  const node = chunk_nodes.get(hit.chunk);
  if (!node) return null;
  return entry_id_of(node.object_key, hit.band, hit.pos);
}
