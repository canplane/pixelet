// @pxl/web/feature/search/poi-badge.ts

import { CATEGORY_BADGE, DEFAULT_BADGE } from "./poi_badge.spec.ts";

/**
 * Category -> badge for a search result, DEFAULT_BADGE for anything unlisted.
 * Emoji, not glyphs: a thin stroked icon at this size read far worse
 * (tried and reverted 2026-09-08).
 */

interface PoiBadge {
  icon: string;
  color: string;
}

export function resolvePoiBadge(category: string): PoiBadge {
  return (CATEGORY_BADGE as Record<string, PoiBadge>)[category] ?? DEFAULT_BADGE;
}
