// @pxl/api/bake/geocode/provider.ts

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

/**
 * One administrative division in the label, broad-to-narrow order (see
 * GeocodeResult's own doc comment for why order is fixed here but display
 * language isn't). `name` is Overture's own `names.primary` — always the
 * local-language form, always present. `common` is a subset of Overture's
 * `names.common` (ingest_divisions.ts's own SUPPORTED_LANGS filter) keyed by
 * language tag — present only when Overture actually had a translation for
 * this division; absent entirely for a division with no translations
 * rather than an empty object.
 *
 * `approx` marks a part resolved from `overture_division_points` (the
 * nearest named point within a radius) rather than true point-in-polygon
 * containment against `overture_divisions` — used only when no polygon at
 * any of the fine-grained subtypes covers the point (overture_provider.ts's
 * own top comment explains why that's common: most of Korea's localities
 * have no mapped boundary at all). Not currently surfaced in the UI, but a
 * real distinction worth keeping around rather than silently blurring
 * "exact" and "nearest-guess" into the same shape.
 */
export type GeocodePart = { name: string; common?: Record<string, string>; approx?: true };

/**
 * Broad-to-narrow, e.g. `[{name:"강남구"}, {name:"역삼1동"}]` — a fixed
 * canonical order this type commits to regardless of any viewer's own
 * language, since it's baked once per page and served to every viewer.
 * Broad-to-narrow vs. narrow-to-broad display order is a language
 * convention (Korean/Japanese/Chinese go broad-to-narrow, most others
 * reverse it — the same pattern as date formats), not a property of the
 * *place* itself, so joining these into one display string, AND picking
 * which language's name to show per part, is the client's job
 * (use_reverse_geocode.ts), done against whatever UI language is actually
 * active — not something to decide once here for every viewer.
 */
export type GeocodeResult = { parts: GeocodePart[] };

/**
 * Reverse geocoding's own PoiProvider-shaped seam (provider.ts's own doc
 * comment for the POI case explains the reasoning; same one applies here) —
 * one page_idx in, that page's neighborhood label out. service.ts (the live
 * endpoint) and bake/geocode/tools/bake_geocode_tiles.ts (the offline
 * static bake) both depend on this, not on any one implementation.
 *
 * providers/overture_provider.ts (Overture's Divisions theme) is the
 * current and, as of 2026-09-08, only implementation — this replaced a
 * direct client-side call to public Nominatim (use_reverse_geocode.ts,
 * now retired) that had no server, no cache, and no bake step at all. See
 * discussions/20260908-overture-poi-adoption.md.
 */
export type GeocodeProvider = {
  reverse(page_idx: IVec): Promise<GeocodeResult | null>;
};
