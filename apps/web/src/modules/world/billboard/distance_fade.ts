// @pxl/web/modules/world/billboard/distance_fade.ts

/**
 * Distance-based scale/opacity for a world-anchored billboard (tier 3, see
 * discussions/20260907-world-rendering-tiers.md). `fit` cancels out
 * perspective so that one logical canvas unit (`canvas_scale.ts`'s own
 * `WORLD_UNITS_PER_PX`) always maps to exactly one on-screen CSS pixel —
 * a consumer just draws its own canvas as if it were a real HTML element at
 * real pixel sizes, and it renders at that size, whatever it draws.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import { WORLD_UNITS_PER_PX } from "./canvas_scale.ts";

/** Full opacity/size at or inside FAR_START, smoothstepped to gone by
 * FAR_END. */
export const FAR_START: f64 = 120;
export const FAR_END: f64 = 320;

export type DistanceFadeResult = {
  /** Multiplies the sprite's own full-strength width/height. */
  scale: f64;
  /** Multiplies the sprite material's opacity — driven by the far fade only. */
  opacity: f64;
};

/** `px_per_world_unit`: CSS pixels per world unit [u] at `dist`, for the
 * live camera — the caller's own job to compute. */
export function distance_fade_factor(px_per_world_unit: f64, dist: f64): DistanceFadeResult {
  const fit = px_per_world_unit > 0 ? 1 / (WORLD_UNITS_PER_PX * px_per_world_unit) : 1;

  let far: f64;
  if (dist <= FAR_START) far = 1;
  else if (dist >= FAR_END) far = 0;
  else {
    const t = (dist - FAR_START) / (FAR_END - FAR_START);
    far = 1 - t * t * (3 - 2 * t); // smoothstep
  }

  return { scale: fit * far, opacity: far };
}
