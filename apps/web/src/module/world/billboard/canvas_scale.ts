// @pxl/web/module/world/billboard/canvas_scale.ts

/**
 * Canvas-drawing constants shared by every world-anchored billboard
 * consumer (chat_bubble.tsx, poi_layer.tsx) so they stay matched by
 * construction instead of each picking its own.
 */

/** Matches apps/web/src/app/style/index.css's own body font-family. Each
 * consumer still picks its own size/weight. */
export const FONT_FAMILY = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "SF Pro Display", "Pretendard Variable", system-ui, sans-serif';

/** Mirrors the app's own HTML-side weight scale (Tailwind's font-normal /
 * font-semibold) rather than canvas's "bold" keyword, which resolves to
 * 700 — Pretendard Variable's weight axis (fonts.css: 45–920) renders
 * either as a real distinct face, not a synthetic bold. */
export const FONT_WEIGHT_REGULAR = 400;
export const FONT_WEIGHT_BOLD = 600;
