// @pxl/web/feature/search/poi-theme.ts

import { CATEGORY_THEME, DEFAULT_THEME } from "./poi-theme-spec.ts";

/**
 * Category -> icon/color lookup for poi_layer.tsx's badge — the one visual
 * idea worth keeping from the pre-DoD POI implementation
 * (`checkpoint/20260107-float`'s `entities/geo/ui/pois/poi-theme-config.ts`)
 * now that POI is back (2026-09-07, see
 * issue/20260907-world-rendering-tiers.md). That version's own
 * day/night variants and hover glow aren't ported here — this pass only
 * brings back "a POI reads as its own kind at a glance," not the full
 * interactive badge; see poi_layer.tsx's own top doc comment for what's
 * still deferred.
 *
 * Keyed off `POI.category` (overture_provider.ts's `category_of()`, which
 * maps Overture's `categories.primary` through OVERTURE_CATEGORY_MAP into
 * this table's own vocabulary), falling back to DEFAULT_THEME for anything
 * not explicitly listed rather than guessing a mapping.
 *
 * `icon` is a plain emoji — tried swapping these for tinted Lucide glyphs
 * (2026-09-08) and reverted: at this badge's actual on-screen size a thin
 * stroked icon reads far less recognizably than an emoji's own dense,
 * multi-shape glyph does, and that shape-level legibility (not stroke
 * weight, tried thickening it first) was the real problem.
 */

export type PoiTheme = { icon: string; color: string };

export function resolve_poi_theme(category: string): PoiTheme {
  return (CATEGORY_THEME as Record<string, PoiTheme>)[category] ?? DEFAULT_THEME;
}
