// @pxl/web/modules/world/billboard/canvas_scale.ts

/**
 * Canvas-drawing constants shared by every world-anchored billboard
 * consumer (chat_bubble.tsx, poi_layer.tsx) so they stay matched by
 * construction instead of each picking its own.
 */

import type { f64 } from "@xpute/core/kit/word.ts";

/** World units [u] per logical (pre-supersampling) canvas pixel — one
 * logical unit maps to exactly one on-screen CSS pixel whenever a billboard
 * is fully visible (see distance_fade.ts). */
export const WORLD_UNITS_PER_PX: f64 = 0.1;

/** Matches apps/web/src/app/styles/index.css's own body font-family. Each
 * consumer still picks its own size/weight. */
export const FONT_FAMILY = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Pretendard Variable", system-ui, sans-serif';
