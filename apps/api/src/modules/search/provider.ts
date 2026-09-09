// @pxl/api/modules/search/provider.ts

import type { GeocodePart } from "@pxl/api/bake/geocode/provider.ts";

/**
 * One matched place — WGS84 lon/lat, not a page_idx or V3: unlike POI/
 * geocode (both keyed to a finite L3 page a client already has open),
 * search's own input space is unbounded free text (see
 * discussions/20260908-overture-poi-adoption.md's own note on why this is
 * the one feature that can't be baked ahead of time) and its output needs
 * to tell the client exactly where to fly the camera to, not which page to
 * look up.
 */
export type SearchResult = {
  gers_id: string;
  /** "poi" — an Overture Places business/venue (`overture_places`).
   * "division" — an administrative area matched by name (region/county/
   * locality/…, `overture_divisions`/`overture_division_points` — the same
   * tables reverse geocoding reads). Searching "서울특별시" or "수내동"
   * only ever matched POI names containing that substring before this
   * existed (a hospital literally named "서울특별시 보라매병원", not the
   * city itself) — this is what actually answers that kind of query.
   * `lon`/`lat` for a division is its polygon's centroid (or the bare
   * point when only `overture_division_points` has it, per geocode's own
   * point-fallback) — deliberately NOT a "zoom out to fit the whole area"
   * target; jumping there behaves exactly like a POI jump (same camera
   * position, same zoom), just centered on a coarser point. */
  kind: "poi" | "division";
  name: string;
  category: string;
  /** Overture's own `addresses[0]` (locality + freeform street), shown
   * exactly as Overture wrote it — no reformatting/cleanup, since it's
   * genuinely freeform per-source text (see providers/overture_provider.ts's
   * own address_of() doc comment: different upstream contributors format
   * this completely differently, confirmed live, and there's no principled
   * way to normalize it without actually parsing addresses). `null` when
   * Overture had no address data for this place, and always null for a
   * "division" result (it has no address of its own, `common` is its
   * multilingual data instead). */
  address: string | null;
  /** A language-tag-keyed name map — populated for `kind: "division"`
   * (Overture's Divisions theme has genuinely rich per-language names,
   * unlike Places — see ingest_divisions.ts's own top comment), always
   * `null` for `kind: "poi"` (Overture Places' own names.common is ~0%
   * populated for Korean places, confirmed this session). Mirrors
   * GeocodePart's own `common` field (geocode/provider.ts) exactly, so the
   * client can reuse the same "pick a language against i18n.language, fall
   * back to `name`" logic use_reverse_geocode.ts already has. */
  common: Record<string, string> | null;
  /** The broader divisions (region/county) containing this result's own
   * point — geocode/resolve_broad.ts's own doc comment on why: a division
   * result like "수내동" is easy to misread without knowing which
   * 성남시/분당구 it's actually in, the same way a POI's `address` answers
   * "which 스타벅스". Always `[]` for `kind: "poi"` (a POI's `address`
   * already covers this, no need to compute it twice two different ways). */
  context: GeocodePart[];
  lon: number;
  lat: number;
};

/**
 * Forward search's own provider seam — same shape as PoiProvider/
 * GeocodeProvider (their own doc comments explain the reasoning), one
 * query in, ranked matches out. providers/overture_provider.ts (trigram
 * search over `overture_places.name`, plain ILIKE over the much smaller
 * `overture_divisions`/`overture_division_points`) is the current and only
 * implementation.
 *
 * `near` is an optional WGS84 point to bias ranking toward — without it, a
 * query like a chain name ("스타벅스") has no principled way to prefer one
 * of its many identically-named locations over another. Only applied to
 * `kind: "poi"` results — a division match is rare enough (and usually
 * unique enough by name) that proximity bias isn't worth the extra query
 * shape for it too.
 */
export type SearchProvider = {
  search(query: string, opts?: { near?: { lon: number; lat: number }; limit?: number }): Promise<SearchResult[]>;
};
