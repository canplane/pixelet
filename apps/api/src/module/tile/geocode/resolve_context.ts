// @pxl/api/module/tile/geocode/resolve_context.ts
//
// Shared with search/provider/overture_provider.ts: a division search
// result (e.g. 수내동) is easy to misread without its own broader context —
// there's more than one 수내동-shaped name nationwide in principle, and
// even when there isn't, "which 성남시/분당구 is this" is exactly what a
// user picking a search result actually wants to know, the same way a
// POI result's own `address` answers "which 스타벅스". This is the
// region/county half of geocode/provider/overture_provider.ts's own
// `reverse()` — factored out because search needs it for an arbitrary
// point (a division's own centroid), not just a page's center, and
// doesn't need that function's finer-grained locality/point-fallback
// logic (a division match already IS the fine level; showing its own
// broader parents is enough).

import type { Pool } from "@db/postgres";

import type { GeocodePart } from "@pxl/api/bake/geocode/provider.ts";

const BROAD_SUBTYPE_ORDER: Record<string, number> = { region: 0, county: 1 };

// Overture's four sub-city subtypes, most-specific-for-Korea first — the
// same ladder geocode/provider/overture_provider.ts walks, kept identical
// so a search result and the current-location label never disagree about
// what the fine level of a place is called.
const FINE_SUBTYPE_PRIORITY = ["locality", "neighborhood", "microhood", "macrohood"];

// `gers_id <> $3` matters when the point being resolved is itself a
// region/county centroid (e.g. resolving context for a "성남시" match) —
// without it, a county's own polygon (which contains its own centroid)
// would show up as its own parent.
const CONTAINING_BROAD_SQL = `SELECT subtype, name, names_common
FROM overture_divisions
WHERE subtype IN ('region', 'county') AND gers_id <> $3
  AND ST_Contains(geom, ST_SetSRID(ST_MakePoint($1, $2), 3857))`;

type Row = { subtype: string; name: string; names_common: Record<string, string> | null };

function to_part(r: Row): GeocodePart {
  return r.names_common ? { name: r.name, common: r.names_common } : { name: r.name };
}

/** `cx`/`cy` in EPSG:3857 meters (same units `overture_divisions.geom` and
 * `overture_division_points.geom` are stored in) — not WGS84 lon/lat.
 * `exclude_gers_id` is the division being resolved itself, so it never
 * shows up as its own parent. */
export async function resolve_broad_context(pool: Pool, cx: number, cy: number, exclude_gers_id: string): Promise<GeocodePart[]> {
  const client = await pool.connect();
  let rows: Row[];
  try {
    const res = await client.queryObject<Row>({ text: CONTAINING_BROAD_SQL, args: [cx, cy, exclude_gers_id] });
    rows = res.rows;
  } finally {
    client.release();
  }

  return order_broad(rows);
}

/** Broad before narrow, each name once. Shared by both resolvers so a
 * batched answer and a single one cannot order themselves differently. */
function order_broad(rows: readonly Row[]): GeocodePart[] {
  const ordered = rows
    .filter((r) => r.subtype in BROAD_SUBTYPE_ORDER)
    .sort((a, b) => BROAD_SUBTYPE_ORDER[a.subtype] - BROAD_SUBTYPE_ORDER[b.subtype]);

  const seen = new Set<string>();
  const parts: GeocodePart[] = [];
  for (const r of ordered) {
    if (seen.has(r.name)) continue;
    seen.add(r.name);
    parts.push(to_part(r));
  }
  return parts;
}

/**
 * The same question for many points at once, and down to the fine level.
 *
 * One statement for the whole set: search resolves context for a result
 * page, and twenty round trips to answer twenty containments is twenty
 * times the latency for the same index scan. `overture_divisions.geom`
 * carries a GiST index (ingest_divisions.ts), so each point is a lookup
 * rather than a scan.
 *
 * The subtype list is the answer's own shape rather than a tuning choice:
 * these six are what the ordering below can use, so joining the rest fetches
 * rows to discard. `country` is the one worth naming — its polygon is the
 * whole of Korea, and an exact containment against it is not cheap.
 *
 * Region and county then one fine level, matching what geocode's own
 * `reverse()` assembles — the fine subtypes are alternatives rather than a
 * chain, so the most specific one that matched wins and the rest are
 * dropped. What this does not do is `reverse()`'s nearest-point fallback: a
 * point with no containing fine polygon gets the coarser label instead of a
 * second query, which for telling two same-named search results apart is
 * already enough.
 */
const CONTAINING_DIVISIONS_BATCH_SQL = `SELECT p.idx, d.subtype, d.name, d.names_common
FROM unnest($1::float8[], $2::float8[]) WITH ORDINALITY AS p(x, y, idx)
JOIN overture_divisions d
  ON d.subtype IN ('region', 'county', 'locality', 'neighborhood', 'microhood', 'macrohood')
 AND ST_Contains(d.geom, ST_SetSRID(ST_MakePoint(p.x, p.y), 3857))`;

type BatchRow = Row & { idx: string };

export async function resolve_point_context_batch(pool: Pool, xs: readonly number[], ys: readonly number[]): Promise<GeocodePart[][]> {
  const out: GeocodePart[][] = xs.map(() => []);
  if (xs.length === 0) return out;

  const client = await pool.connect();
  let rows: BatchRow[];
  try {
    const res = await client.queryObject<BatchRow>({ text: CONTAINING_DIVISIONS_BATCH_SQL, args: [xs, ys] });
    rows = res.rows;
  } finally {
    client.release();
  }

  const by_point = new Map<number, Row[]>();
  for (const r of rows) {
    const i = Number(r.idx) - 1;
    const list = by_point.get(i) ?? [];
    list.push(r);
    by_point.set(i, list);
  }

  for (const [i, list] of by_point) {
    const parts = order_broad(list);
    const fine = FINE_SUBTYPE_PRIORITY
      .map((subtype) => list.find((r) => r.subtype === subtype))
      .find((r): r is Row => r !== undefined);
    if (fine && !parts.some((p) => p.name === fine.name)) parts.push(to_part(fine));
    out[i] = parts;
  }
  return out;
}
