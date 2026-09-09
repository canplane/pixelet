// @pxl/api/modules/world/geocode/providers/overture_provider.ts

import type { Pool } from "@db/postgres";

import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import type { GeocodePart, GeocodeProvider, GeocodeResult } from "../provider.ts";

// region (도/특별시) → county (시/구) — GeocodeResult's own canonical
// broad-to-narrow storage order (see provider.ts's doc comment on why this
// is fixed here regardless of display language). Sorted strictly by
// subtype rather than by name pattern or polygon size: Korea's own
// administrative depth isn't uniform (ingest_divisions.ts's own top
// comment — Seoul has 3 real levels under `region`, a split city like
// Seongnam also has 3 but `county` lands on the city, not a district), so
// this always renders whatever's actually the finest broad-level data
// Overture has for a given point rather than assuming a fixed real-world
// meaning per subtype.
const BROAD_SUBTYPE_ORDER: Record<string, number> = { region: 0, county: 1 };

// Overture's four sub-city subtypes, most-specific-for-Korea first: a
// formal 동 (`locality`) is what most reverse-geocode consumers actually
// want, so it wins over an informal/commercial area sharing the same
// point even when both have a polygon there. Only the FIRST matching
// subtype is used as `fine_from_polygon` below — these aren't summed like
// region/county are, since unlike region⊃county, Overture doesn't
// guarantee these four nest cleanly (a `microhood` can be a business
// district that overlaps a `locality` without being contained by any real
// hierarchy).
const FINE_SUBTYPE_PRIORITY = ["locality", "neighborhood", "microhood", "macrohood"];

// How far to look for a fallback `division` point when nothing narrower is
// available any other way (no polygon at all covers the point — true edge
// of Overture's Divisions coverage). Constrained lookups (inside an
// already-matched polygon, see best_containing_id below) don't need this
// radius at all; it only bounds the last-resort unconstrained search. 2km
// comfortably covers a typical dong's radius without reaching into a
// genuinely different one; a false match beyond that would misname the
// area rather than just be imprecise about its own boundary.
const FINE_POINT_RADIUS_M = 2000;

const CONTAINING_DIVISIONS_SQL = `SELECT gers_id, subtype, name, names_common
FROM overture_divisions
WHERE ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857))`;

// Nearest named `division` point actually inside a specific already-matched
// polygon (by gers_id) — used to find a finer label (e.g. 백현동) nested
// inside a coarser one we already resolved (e.g. 분당구, itself classified
// `locality` — see ingest_divisions.ts's own top comment on why Overture's
// subtype ladder doesn't track Korea's real per-city depth). Constraining
// to "inside this exact polygon" rather than a bare radius avoids picking
// a point that's merely nearby but across a real administrative boundary.
const NEAREST_POINT_INSIDE_SQL = `SELECT p.subtype, p.name, p.names_common
FROM overture_division_points p
JOIN overture_divisions d ON d.gers_id = $3
WHERE ST_Contains(d.geom, p.geom)
ORDER BY p.geom <-> ST_SetSRID(ST_MakePoint($1, $2), 3857)
LIMIT 1`;

// Last-resort unconstrained search, only reached when NO polygon at all
// (not even region/county) covers the point.
const NEAREST_POINT_IN_RADIUS_SQL = `SELECT subtype, name, names_common
FROM overture_division_points
WHERE ST_DWithin(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857), $3)
ORDER BY geom <-> ST_SetSRID(ST_MakePoint($1, $2), 3857)
LIMIT 1`;

type AreaRow = { gers_id: string; subtype: string; name: string; names_common: Record<string, string> | null };
type PointRow = { subtype: string; name: string; names_common: Record<string, string> | null };

function to_part(r: AreaRow | PointRow, approx?: true): GeocodePart {
  const part: GeocodePart = r.names_common ? { name: r.name, common: r.names_common } : { name: r.name };
  if (approx) part.approx = true;
  return part;
}

/**
 * Overture's Divisions theme resolved by point-in-polygon against the
 * page's own center point (`overture_divisions`, region/county/locality/
 * neighborhood/microhood/macrohood subtypes — ingest_divisions.ts's own
 * top comment on why `country` is the only one dropped). Because most of
 * Korea's finest-level localities have NO mapped polygon at all (same top
 * comment), this always additionally looks for the nearest named
 * `division` point (`overture_division_points`) nested inside whatever
 * polygon it already matched, and appends it as an extra, finer part when
 * its name is new — e.g. 분당구 (a real `locality` polygon) plus 백현동 (only
 * ever a point) both end up in the label, not just whichever one happened
 * to have a polygon. One label per page, not per exact coordinate: reverse
 * geocoding is baked at the same L3 granularity terrain/POI already are,
 * and a division polygon (or the point-search radius) is always far larger
 * than one page regardless.
 *
 * Divisions is OSM-derived (ODbL) inside Overture, unlike Places — see
 * discussions/20260908-overture-poi-adoption.md's own note that this
 * adopts Overture here for its schema, not to escape OSM licensing the way
 * the POI switch did.
 */
export function make_overture_geocode_provider(pool: Pool): GeocodeProvider {
  return {
    async reverse(page_idx: IVec): Promise<GeocodeResult | null> {
      const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
      const cx = (page_bbox[0][0] + page_bbox[1][0]) / 2;
      const cy = (page_bbox[0][1] + page_bbox[1][1]) / 2;

      const client = await pool.connect();
      let rows: AreaRow[];
      let fine_point: PointRow | null = null;
      let fine_from_polygon: AreaRow | null = null;
      try {
        const res = await client.queryObject<AreaRow>({ text: CONTAINING_DIVISIONS_SQL, args: [cx, cy] });
        rows = res.rows;

        fine_from_polygon = FINE_SUBTYPE_PRIORITY
          .map((subtype) => rows.find((r) => r.subtype === subtype))
          .find((r): r is AreaRow => r !== undefined) ?? null;

        // Prefer constraining the point search to the smallest polygon we
        // already matched (fine subtype first, then county, then region —
        // region is a last resort since "somewhere in this province" barely
        // narrows anything down but still beats an unconstrained search).
        const county_row = rows.find((r) => r.subtype === "county") ?? null;
        const region_row = rows.find((r) => r.subtype === "region") ?? null;
        const containing_id = fine_from_polygon?.gers_id ?? county_row?.gers_id ?? region_row?.gers_id ?? null;

        const near = containing_id !== null
          ? await client.queryObject<PointRow>({ text: NEAREST_POINT_INSIDE_SQL, args: [cx, cy, containing_id] })
          : await client.queryObject<PointRow>({ text: NEAREST_POINT_IN_RADIUS_SQL, args: [cx, cy, FINE_POINT_RADIUS_M] });
        fine_point = near.rows[0] ?? null;
      } finally {
        client.release();
      }

      // Overture's own Divisions extract has occasional overlapping
      // same-named polygons at a given subtype (confirmed live: two
      // distinct `county` rows both named "옹진군" containing the same
      // point, and separately two `region` rows both named "경기도"
      // distinguished only by a `class` of land vs. maritime) — a
      // data-quality artifact upstream, not a bug in this query. Deduping
      // by name (not by row) is the cheap, always-correct fix: a real
      // label never needs the same name twice regardless of why it showed
      // up twice.
      const broad = rows
        .filter((r) => r.subtype in BROAD_SUBTYPE_ORDER)
        .sort((a, b) => BROAD_SUBTYPE_ORDER[a.subtype] - BROAD_SUBTYPE_ORDER[b.subtype]);

      const seen = new Set<string>();
      const parts: GeocodePart[] = [];
      for (const r of broad) {
        if (seen.has(r.name)) continue;
        seen.add(r.name);
        parts.push(to_part(r));
      }
      if (fine_from_polygon && !seen.has(fine_from_polygon.name)) {
        seen.add(fine_from_polygon.name);
        parts.push(to_part(fine_from_polygon));
      }
      // Appended, not substituted: a real polygon match (fine_from_polygon)
      // and a nested point-only place (fine_point) name two DIFFERENT real
      // levels (분당구 vs. 백현동), both worth showing — this is exactly the
      // case a single fine-subtype slot couldn't represent before.
      if (fine_point && !seen.has(fine_point.name)) {
        parts.push(to_part(fine_point, true));
      }

      return parts.length > 0 ? { parts } : null;
    },
  };
}
