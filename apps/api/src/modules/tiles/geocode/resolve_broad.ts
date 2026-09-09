// @pxl/api/modules/tiles/geocode/resolve_broad.ts
//
// Shared with search/providers/overture_provider.ts: a division search
// result (e.g. 수내동) is easy to misread without its own broader context —
// there's more than one 수내동-shaped name nationwide in principle, and
// even when there isn't, "which 성남시/분당구 is this" is exactly what a
// user picking a search result actually wants to know, the same way a
// POI result's own `address` answers "which 스타벅스". This is the
// region/county half of geocode/providers/overture_provider.ts's own
// `reverse()` — factored out because search needs it for an arbitrary
// point (a division's own centroid), not just a page's center, and
// doesn't need that function's finer-grained locality/point-fallback
// logic (a division match already IS the fine level; showing its own
// broader parents is enough).

import type { Pool } from "@db/postgres";

import type { GeocodePart } from "@pxl/api/bake/geocode/provider.ts";

const BROAD_SUBTYPE_ORDER: Record<string, number> = { region: 0, county: 1 };

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
