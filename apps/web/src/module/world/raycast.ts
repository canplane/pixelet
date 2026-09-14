// @pxl/web/module/world/raycast.ts

/**
 * The world's raycast, in the module (crates/pxl-core/src/world/raycast.rs):
 * `ray_hit_world` asks it, over the chunks the scheduler holds and the
 * composites it keeps, and reads the hit back as doubles. The TypeScript
 * walk the module's is held to is reference/chunk_raycast.ts.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { CORE_PXL } from "@/wasm/core_pool.ts";
import type { MaterialClass } from "@/render/mesh/voxel.ts";

export type WorldHit = {
  material_class: MaterialClass;
  face_normal: V3;
  hit_point: V3;
  t: f64;
  box_min: V3;
  box_max: V3;
};

/** A ray as an origin and a direction, structurally: what camera/ray.ts
 * builds from the view and what the reference walk takes. */
export type RayLike = {
  readonly origin: { readonly x: f64; readonly y: f64; readonly z: f64 };
  readonly direction: { readonly x: f64; readonly y: f64; readonly z: f64 };
};

/**
 * The nearest box the ray strikes within `max_dist`, or null: the module's
 * walk of every chunk the ray's XY projection crosses. The ray is in the
 * local world frame — relative to `map_origin` on X and Y — and so is what
 * comes back.
 */
export function ray_hit_world(ray: RayLike, map_origin: IVec, max_dist: f64): WorldHit | null {
  const p = CORE_PXL;
  if (p === null) throw new Error("render: the raycast walks the module's chunks, which needs the core memory");
  const struck = p.pxl_raycast(
    ivec.to_real(ivec.X(map_origin)),
    ivec.to_real(ivec.Y(map_origin)),
    ray.origin.x,
    ray.origin.y,
    ray.origin.z,
    ray.direction.x,
    ray.direction.y,
    ray.direction.z,
    max_dist,
  );
  if (struck === 0) return null;
  const h = new Float64Array(p.memory.buffer, p.pxl_raycast_at(), 14);
  return {
    material_class: h[0] as MaterialClass,
    face_normal: [h[1], h[2], h[3]],
    hit_point: [h[4], h[5], h[6]],
    t: h[7],
    box_min: [h[8], h[9], h[10]],
    box_max: [h[11], h[12], h[13]],
  };
}

/** Ray/AABB slab test. */
