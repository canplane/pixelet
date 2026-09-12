// @pxl/api/module/search/provider/overture_provider.ts

import type { Pool } from "@db/postgres";

import { OVERTURE_CATEGORY_MAP } from "@pxl/api/bake/overture/model/category_map.ts";
import { resolve_broad_context, resolve_point_context_batch } from "../../tile/geocode/resolve_context.ts";
import type { SearchProvider, SearchResult } from "../provider.ts";

const DEFAULT_LIMIT = 20;
const MAX_LIMIT = 50;

// Divisions never crowd out POI results — a handful of admin-area matches
// (there's rarely more than one real "서울특별시") capped well under the
// overall limit, with the rest of the budget going to POI.
const DIVISION_LIMIT = 5;

// `name ILIKE '%'||$1||'%'` is what the GIN gin_trgm_ops index on
// overture_places.name (ingest_places.ts's own schema) actually
// accelerates — trigram's `%` similarity operator alone would reject very
// short Korean queries (a 2-syllable name like "강남" barely forms enough
// trigrams to clear the default 0.3 similarity_threshold), so ILIKE is the
// primary filter and similarity() is only the ranking signal, not the
// WHERE clause.
//
// Two separate statements rather than one with an optional param: ORDER BY
// needs a genuinely different expression when a bias point exists, not
// just a NULL-able argument to the same one.
const SEARCH_SQL = `SELECT gers_id, name, properties,
  ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_places
WHERE name <> '' AND name ILIKE '%' || $1 || '%'
ORDER BY similarity(name, $1) DESC, name
LIMIT $2`;

// Same query, but ranked against `near` — the version that answers "which
// 강남역" and "which 스타벅스", which is what a map search is usually being
// asked.
//
// Similarity is rounded to two decimals before it is ordered on, and that
// rounding is the whole point. Ordering on the raw score made distance a
// tiebreaker for exactly-equal names and nothing else, so "스타벅스 강남대로"
// at 0.61 sorted ahead of every "스타벅스" at 0.60 no matter which continent
// each was on — a difference of one trigram outranking a difference of a
// thousand kilometres. A band collapses scores that are indistinguishable as
// answers into one tie, and distance then decides inside the band. Two
// decimals is roughly "the same name, modulo a word": it does not let a
// genuinely worse match win on proximity, since those differ by far more
// than 0.01.
const SEARCH_NEAR_SQL = `SELECT gers_id, name, properties,
  ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
  ST_X(ST_Transform(geom, 4326)) AS lon, ST_Y(ST_Transform(geom, 4326)) AS lat
FROM overture_places
WHERE name <> '' AND name ILIKE '%' || $1 || '%'
ORDER BY round(similarity(name, $1)::numeric, 2) DESC,
  geom <-> ST_Transform(ST_SetSRID(ST_MakePoint($3, $4), 4326), 3857) ASC
LIMIT $2`;

// overture_divisions/overture_division_points are small (a few thousand
// and tens of thousands of rows respectively, nationwide) — no trigram
// index needed the way overture_places' 691K rows requires one; a plain
// ILIKE scan over both is fast enough as-is.
//
// Area first (`is_point` 0 before 1): a polygon's own centroid is a more
// meaningful "center of this place" than a same-named point elsewhere
// would be, and the caller dedupes by name keeping the first occurrence —
// see dedupe_divisions_by_name below.
// UNION's own ORDER BY can only reference output column names, not
// re-invoke an expression like similarity() directly — confirmed live
// ("invalid UNION/INTERSECT/EXCEPT ORDER BY clause") — so both branches
// compute their own `sim` column and the ORDER BY references that instead.
// x3857/y3857 alongside the WGS84 lon/lat: the client needs lon/lat to
// jump the camera there, but resolve_broad_context (find the region/county
// containing this result) needs the same point in the 3857 meters
// overture_divisions/overture_division_points geometry is actually stored
// in — cheaper to select both once here than reproject client-facing
// coordinates back into 3857 in application code.
const DIVISION_SEARCH_SQL = `
(
  SELECT gers_id, name, subtype, names_common, 0 AS is_point, similarity(name, $1) AS sim,
    ST_X(ST_Centroid(geom)) AS x3857, ST_Y(ST_Centroid(geom)) AS y3857,
    ST_X(ST_Transform(ST_Centroid(geom), 4326)) AS lon,
    ST_Y(ST_Transform(ST_Centroid(geom), 4326)) AS lat
  FROM overture_divisions
  WHERE name <> '' AND name ILIKE '%' || $1 || '%'
)
UNION ALL
(
  SELECT gers_id, name, subtype, names_common, 1 AS is_point, similarity(name, $1) AS sim,
    ST_X(geom) AS x3857, ST_Y(geom) AS y3857,
    ST_X(ST_Transform(geom, 4326)) AS lon,
    ST_Y(ST_Transform(geom, 4326)) AS lat
  FROM overture_division_points
  WHERE name <> '' AND name ILIKE '%' || $1 || '%'
)
ORDER BY is_point ASC, sim DESC
LIMIT $2`;

// lon/lat typed as the strings @db/postgres actually decodes `double
// precision` results into (see the Number() cast below) — not a lie about
// what the driver hands back, unlike leaving these as `number` would be.
type PlaceRow = { gers_id: string; name: string; properties: Record<string, unknown>; x3857: string; y3857: string; lon: string; lat: string };
type DivisionRow = {
  gers_id: string;
  name: string;
  subtype: string;
  names_common: Record<string, string> | null;
  x3857: string;
  y3857: string;
  lon: string;
  lat: string;
};

function overture_category(properties: Record<string, unknown>): string | undefined {
  return (properties.categories as { primary?: string } | undefined)?.primary;
}

function category_of(properties: Record<string, unknown>): string {
  const primary = overture_category(properties);
  return (primary && OVERTURE_CATEGORY_MAP[primary]) ?? primary ?? "place";
}

type OvertureAddress = { region?: string | null; locality?: string | null; freeform?: string | null };
// The same area/point name can legitimately show up twice (a `locality`
// polygon and a `division` point both named "종로구", say) — keeping the
// first occurrence is enough since DIVISION_SEARCH_SQL already orders area
// rows before point rows for exactly this reason.
function dedupe_divisions_by_name(rows: DivisionRow[]): DivisionRow[] {
  const seen = new Set<string>();
  const out: DivisionRow[] = [];
  for (const r of rows) {
    if (seen.has(r.name)) continue;
    seen.add(r.name);
    out.push(r);
  }
  return out;
}

/**
 * Forward search over Overture's Places theme (`overture_places`, same
 * table POI already reads) for `kind: "poi"` results, plus
 * `overture_divisions`/`overture_division_points` (the same tables reverse
 * geocoding reads) for `kind: "division"` — see provider.ts's own doc
 * comment on why a name like "서울특별시"/"수내동" needs both: without the
 * division half, that query only ever matched a POI whose own name
 * happened to contain it. A division's `lon`/`lat` is its polygon
 * centroid (or bare point, when only overture_division_points has one) —
 * plain "jump there," not a "zoom out to fit the whole area" camera
 * behavior, matching how a POI jump already works.
 */
export function make_overture_search_provider(pool: Pool): SearchProvider {
  return {
    async search(query: string, opts?: { near?: { lon: number; lat: number }; limit?: number }): Promise<SearchResult[]> {
      const limit = Math.min(Math.max(opts?.limit ?? DEFAULT_LIMIT, 1), MAX_LIMIT);

      const client = await pool.connect();
      let place_rows: PlaceRow[];
      let division_rows: DivisionRow[];
      try {
        const [places_res, divisions_res] = await Promise.all([
          opts?.near
            ? client.queryObject<PlaceRow>({ text: SEARCH_NEAR_SQL, args: [query, limit, opts.near.lon, opts.near.lat] })
            : client.queryObject<PlaceRow>({ text: SEARCH_SQL, args: [query, limit] }),
          client.queryObject<DivisionRow>({ text: DIVISION_SEARCH_SQL, args: [query, DIVISION_LIMIT * 2] }),
        ]);
        place_rows = places_res.rows;
        division_rows = dedupe_divisions_by_name(divisions_res.rows).slice(0, DIVISION_LIMIT);
      } finally {
        client.release();
      }

      // One extra query per division result (capped at DIVISION_LIMIT, so
      // at most a handful) — see resolve_context.ts's own doc comment for
      // why this needs its own point-in-polygon lookup rather than
      // reusing anything already fetched above.
      const contexts = await Promise.all(
        division_rows.map((r) => resolve_broad_context(pool, Number(r.x3857), Number(r.y3857), r.gers_id)),
      );

      // @db/postgres decodes `double precision` results as strings, not JS
      // numbers (confirmed live) — harmless where callers only ever do
      // arithmetic on a value (auto-coerces), but this goes straight into
      // a public JSON response, where a quoted "127.03..." instead of a
      // bare number would be a real, silent API contract bug.
      const division_results: SearchResult[] = division_rows.map((r, i) => ({
        gers_id: r.gers_id,
        kind: "division",
        name: r.name,
        category: r.subtype,
        address: null,
        common: r.names_common,
        context: contexts[i],
        lon: Number(r.lon),
        lat: Number(r.lat),
      }));
      // Where the point actually is, rather than what a text field on the
      // row says about it.
      //
      // Overture's `addresses[0]` is contributor-entered and the geometry is
      // not. There is a row named 강남역 carrying the address "Seoul
      // 서울특별시 강남구" whose coordinates are in 용인, so the list said
      // Seoul and picking it moved the camera to 용인. That is not fixable
      // in the address string.
      //
      // Nor is the inconsistency, which is the larger half: the same field
      // arrives as "서울 서초동 1319-13", as "Seoul Seocho-daero 398
      // Seocho-gu, Seoul, Korea", as "Seoul", and as null, because each is
      // however a different contributor chose to write it. Resolving the
      // point instead gives every result the same shape in the reader's own
      // language, which is what made the address field's remaining job
      // disappear rather than move.
      const poi_context = await resolve_point_context_batch(
        pool,
        place_rows.map((r) => Number(r.x3857)),
        place_rows.map((r) => Number(r.y3857)),
      );
      const poi_results: SearchResult[] = place_rows.map((r, i) => ({
        gers_id: r.gers_id,
        kind: "poi",
        name: r.name,
        category: category_of(r.properties),
        common: null,
        context: poi_context[i],
        lon: Number(r.lon),
        lat: Number(r.lat),
      }));

      // Divisions first: a query matching an admin-area name is rare
      // enough to almost always be the point, worth surfacing ahead of
      // whatever POI incidentally contains the same substring.
      return [...division_results, ...poi_results].slice(0, limit);
    },
  };
}
