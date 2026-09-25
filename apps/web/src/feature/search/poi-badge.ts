// @pxl/web/feature/search/poi-badge.ts

import { CATEGORY_BADGE, DEFAULT_BADGE } from "./poi_badge.spec.ts";

/**
 * Category -> icon/color lookup for a place in the search results, so a
 * place reads as its own kind at a glance.
 *
 * Keyed off a result's category (pxl-geo-spec overture/properties.rs's
 * `category_of()`, which
 * maps Overture's `categories.primary` through OVERTURE_CATEGORY_MAP into
 * this table's own vocabulary), falling back to DEFAULT_BADGE for anything
 * not explicitly listed rather than guessing a mapping.
 *
 * `icon` is a plain emoji — tried swapping these for tinted Lucide glyphs
 * (2026-09-08) and reverted: at this badge's actual on-screen size a thin
 * stroked icon reads far less recognizably than an emoji's own dense,
 * multi-shape glyph does, and that shape-level legibility (not stroke
 * weight, tried thickening it first) was the real problem.
 */

export interface PoiBadge {
  icon: string;
  color: string;
}

export function resolvePoiBadge(category: string): PoiBadge {
  return (CATEGORY_BADGE as Record<string, PoiBadge>)[category] ?? DEFAULT_BADGE;
}
