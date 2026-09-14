// @pxl/web/module/world/billboard/distance_fade.ts

/**
 * The distance fade of a world-anchored billboard (tier 3, see
 * issue/20260907-world-rendering-tiers.md), on the CPU: what the badge
 * shader (render/gpu/badges.ts) shrinks and dims a billboard by, for
 * anything that reasons about the screen box a billboard will take. A
 * billboard at full strength is drawn at one screen pixel per logical
 * pixel, so its drawn width is `logical_w * far`.
 */

import type { f64 } from "@xpute/core/kit/word.ts";
import { fade_end_u, fade_start_u } from "@/module/world/lod.ts";

/** Full opacity/size at or inside fade_start_u(), smoothstepped to gone by
 * fade_end_u() — both a fixed number of the terrain's first tier handoff
 * (module/world/lod.ts), so they move with the projection as the tiers do. */
export { fade_end_u, fade_start_u };

export function distance_fade_far(dist: f64): f64 {
  const start = fade_start_u(), end = fade_end_u();
  if (dist <= start) return 1;
  if (dist >= end) return 0;
  const t = (dist - start) / (end - start);
  return 1 - t * t * (3 - 2 * t); // smoothstep
}
