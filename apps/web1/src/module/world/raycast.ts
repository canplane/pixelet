// @pxl/web/module/world/raycast.ts

/**
 * The world's raycast, in the module (crates/pxl/core/src/world/raycast.rs):
 * `ray_hit_world` asks it, over the chunks the scheduler holds and the
 * composites it keeps, and reads the hit back as doubles. The TypeScript
 * walk the module's is held to is reference/chunk_raycast.ts.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import type { F64Array, U8Array } from "@xpute/core/kit/array.ts";
import type { Vec3 } from "@xpute/core/kit/mat4.ts";
import { encoder, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import { Cmd } from "@pxl/shared/spec/cmd.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";

import { CORE_BUILD } from "@/wasm/core_build.ts";
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
  return CORE_BUILD.call(Cmd.RAY_READ, ray_packet(map_origin, ray.origin, ray.direction, max_dist), (struck, reply) => {
    if (struck !== 1 || reply === null) return null;
    const h = reply.at(0).get<F64Array>();
    return {
      material_class: h[0] as MaterialClass,
      face_normal: [h[1], h[2], h[3]],
      hit_point: [h[4], h[5], h[6]],
      t: h[7],
      box_min: [h[8], h[9], h[10]],
      box_max: [h[11], h[12], h[13]],
    };
  });
}

const $ray = new Float64Array(9);

/** A ray as a ray query carries it (@pxl/shared/spec/cmd.ts): the map
 * origin, then the ray, then the distance. */
export function ray_packet(map_origin: IVec, origin: Vec3, direction: Vec3, max_dist: f64): U8Array {
  $ray[0] = ivec.to_real(ivec.X(map_origin));
  $ray[1] = ivec.to_real(ivec.Y(map_origin));
  $ray[2] = origin.x;
  $ray[3] = origin.y;
  $ray[4] = origin.z;
  $ray[5] = direction.x;
  $ray[6] = direction.y;
  $ray[7] = direction.z;
  $ray[8] = max_dist;
  return encoder.encode(new TreeView().branch((b) => b.f64_array($ray)));
}

/** Ray/AABB slab test. */
