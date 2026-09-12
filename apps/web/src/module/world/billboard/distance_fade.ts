// @pxl/web/module/world/billboard/distance_fade.ts

/**
 * Distance-based scale/opacity for a world-anchored billboard (tier 3, see
 * issue/20260907-world-rendering-tiers.md). `fit` cancels out
 * perspective so that one logical canvas unit (`canvas_scale.ts`'s own
 * `WORLD_UNITS_PER_PX`) always maps to exactly one on-screen CSS pixel —
 * a consumer just draws its own canvas as if it were a real HTML element at
 * real pixel sizes, and it renders at that size, whatever it draws.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import { fade_end_u, fade_start_u } from "@/render/lod.ts";
import { WORLD_UNITS_PER_PX } from "./canvas_scale.ts";

/** Full opacity/size at or inside fade_start_u(), smoothstepped to gone by
 * fade_end_u() — both a fixed number of the terrain's first tier handoff
 * (render/lod.ts), so they move with the projection as the tiers do. */
export { fade_end_u, fade_start_u };

export type DistanceFadeResult = {
  /** Multiplies the sprite's own full-strength width/height. */
  scale: f64;
  /** Multiplies the sprite material's opacity — driven by the far fade only. */
  opacity: f64;
};

/** `px_per_world_unit`: CSS pixels per world unit [u] at `dist`, for the
 * live camera — the caller's own job to compute. */
/**
 * The distance half of the fade, on its own.
 *
 * It is also the entire screen-size story, which is not obvious and is worth
 * having in one place: a billboard's drawn width is
 * `logical_w * WORLD_UNITS_PER_PX * scale * px_per_world_unit`, and `fit`
 * below is the reciprocal of the first and last of those — so it all
 * collapses to `logical_w * far`. That is what `fit` is *for*, and it means
 * anything reasoning about screen boxes needs this and not a projection.
 */
export function distance_fade_far(dist: f64): f64 {
  const start = fade_start_u(), end = fade_end_u();
  if (dist <= start) return 1;
  if (dist >= end) return 0;
  const t = (dist - start) / (end - start);
  return 1 - t * t * (3 - 2 * t); // smoothstep
}

export function distance_fade_factor(px_per_world_unit: f64, dist: f64): DistanceFadeResult {
  const fit = px_per_world_unit > 0 ? 1 / (WORLD_UNITS_PER_PX * px_per_world_unit) : 1;
  const far = distance_fade_far(dist);
  return { scale: fit * far, opacity: far };
}
