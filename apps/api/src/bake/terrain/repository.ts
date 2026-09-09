// @pxl/api/bake/terrain/repository.ts

import { Pool, type PoolClient } from "@db/postgres";

import type { f64, i16 } from "@xpute/core/kit/word.ts";

import type { BaseRow, BuildingRow, CoastRow, SegmentRow } from "@pxl/shared/bake/format.ts";
import { ELE_PAD, ELE_PADDED_SPAN, PAGE_ELE_SPAN, XY_CELL_PER_M } from "@pxl/shared/bake/constants.ts";
import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";

const ESA_ELE_SQL = `WITH cfg AS (
  SELECT
    ${PAGE_ELE_SPAN}::float8 AS ele_span,
    $5::float8 AS z_cell_per_m,
    ${ELE_PAD}::int AS pad,
    ${ELE_PADDED_SPAN}::int AS padded_span
),
params AS (
  SELECT
    $1::float8 AS minx,
    $2::float8 AS miny,
    $3::float8 AS maxx,
    $4::float8 AS maxy,
    ($3::float8 - $1::float8) AS width_m
),
calculated AS (
  SELECT width_m / ele_span AS stride_m
  FROM params, cfg
),
ref_rast AS (
  SELECT ST_MakeEmptyRaster(
    (SELECT padded_span FROM cfg), (SELECT padded_span FROM cfg),
    (SELECT minx - (pad * stride_m) FROM params, cfg, calculated),
    (SELECT maxy + (pad * stride_m) FROM params, cfg, calculated),
    (SELECT stride_m FROM calculated), -(SELECT stride_m FROM calculated),
    0, 0, 3857
  ) AS rast
),
env AS (
  -- Expanded source envelope to keep bilinear sampling valid near edges
  SELECT ST_Expand(ST_Envelope(rast), (SELECT stride_m * 2 FROM calculated)) AS e
  FROM ref_rast
),
mosaic AS (
  -- Clip source rasters to the expanded envelope before union
  SELECT ST_Union(ST_Clip(r.rast, env.e)) AS rast
  FROM esa_elevation_raster r, env
  WHERE r.rast && env.e AND ST_Intersects(r.rast, env.e)
),
resampled AS (
  -- Resample onto the exact target grid
  SELECT ST_Resample(m.rast, r.rast, 'Bilinear') AS rast
  FROM mosaic m, ref_rast r
),
final_rast AS (
  -- Final clip to the exact output extent
  SELECT ST_Clip(s.rast, ST_Envelope(r.rast)) AS rast
  FROM resampled s, ref_rast r
),
dumped AS (
  -- Extract the final resampled cell matrix as a 2D value array
  SELECT (ST_DumpValues(rast)).valarray AS z_matrix
  FROM final_rast
)
SELECT
  array_agg(
    COALESCE(round(val * (SELECT z_cell_per_m FROM cfg)), 0)::int2
    ORDER BY row_idx DESC, col_idx ASC
  ) AS ele
FROM dumped,
     unnest(z_matrix) WITH ORDINALITY AS t(val, ord),
     LATERAL (
       SELECT
         ((ord - 1) / (SELECT padded_span FROM cfg))::int AS row_idx,
         ((ord - 1) % (SELECT padded_span FROM cfg))::int AS col_idx
     ) AS l;`;

const OVERTURE_BUILDINGS_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, subtype, class, height, num_floors, ST_AsBinary(geom) AS geom_wkb
FROM overture_buildings, env
WHERE geom && env.e`;

const OVERTURE_TRANSPORTATION_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, subtype, class, road::text AS road, ST_AsBinary(geom) AS geom_wkb
FROM overture_transportation, env
WHERE geom && env.e`;

// Base is fragmented across four tables (see ingest_base.ts) but shares one
// row shape — a literal `source` column per branch tells overture_adapter.ts
// which table's own subtype/class vocabulary a row came from.
const OVERTURE_BASE_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT gers_id, 'land_use' AS source, subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_land_use, env WHERE geom && env.e
UNION ALL
-- Some water rows are country-scale (up to 444,626 points); clip like OSM_COAST_SQL did. ST_IsEmpty guards && 's bbox-only false positives.
SELECT gers_id, 'water' AS source, subtype, class, ST_AsBinary(clipped) AS geom_wkb FROM (
  SELECT gers_id, subtype, class, ST_Intersection(geom, env.e) AS clipped
  FROM overture_base_water, env
  WHERE geom && env.e
) w WHERE NOT ST_IsEmpty(clipped)
UNION ALL
SELECT gers_id, 'land' AS source, subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_land, env WHERE geom && env.e
UNION ALL
SELECT gers_id, 'infrastructure' AS source, subtype, class, ST_AsBinary(geom) AS geom_wkb FROM overture_base_infrastructure, env WHERE geom && env.e`;

// Overture's Base `water` theme's `ocean` subtype (confirmed via
// docs.overturemaps.org: "created from OpenStreetMap coastlines data
// (natural=coastline) using a QA'd version of the output from the
// OSMCoastline tool" — the same tool family osm_coastline_polygons'
// own land-polygons-split-3857 came from, QA'd for the self-intersection/
// flipped-land-sea errors that tool is known to produce). Overture's own
// geometry represents ocean/water, the *inverse* of what osm_coastline_
// polygons stored (that was land) — so this query inverts it back: union
// whatever ocean touches the page, then take the page envelope's own
// difference against that union, giving a LAND polygon in the exact same
// shape osm_adapter.ts's from_coast_row() already expects (empty result =>
// row dropped => page treated as pure ocean; a result covering the whole
// envelope => dropped as "nothing to carve, already all land"; anything
// else => the real partial coastline geometry to carve BARE out of a
// SEA-filled base). ground_baker.ts/osm_adapter.ts/osm_policy.ts's own
// sea-handling logic is untouched by this — only where the input geometry
// comes from changed.
const OVERTURE_COAST_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
),
ocean_union AS (
  -- Clipped first: ocean rows run up to 163,456 points, and ST_Difference below would otherwise chew through all of them.
  SELECT ST_Union(ST_Intersection(geom, env.e)) AS g FROM overture_base_water, env
  WHERE subtype = 'ocean' AND geom && env.e
),
land AS (
  SELECT COALESCE(ST_Difference(env.e, ocean_union.g), env.e) AS g, env.e AS e
  FROM env LEFT JOIN ocean_union ON true
)
SELECT
  'overture_ocean' AS id,
  ST_AsBinary(g) AS geom_wkb,
  ST_Covers(g, e) AS covers_page
FROM land
WHERE NOT ST_IsEmpty(g)`;

// One PREPAREd statement per query, EXECUTEd with literal args rather than
// re-parsed/re-planned every call. Confirmed live (EXPLAIN ANALYZE): a bbox
// lookup against overture_buildings alone spent ~34ms of its ~44ms in
// planning, not execution — PostgreSQL's own per-backend query planner cost,
// invisible to client-side concurrency or pool sizing. PREPARE once per
// physical connection, then EXECUTE, cut that to ~2ms after the first call
// (confirmed: 49ms -> ~2-3ms across repeated calls on the same connection).
//
// PostgreSQL's EXECUTE doesn't accept its own $-bind parameters the way a
// normal query does (confirmed live: the driver's own parameterized-query
// path errors with "bind message supplies N parameters, but prepared
// statement \"\" requires 0") — its argument list has to be literal
// expressions in the command text itself, hence lit() below rather than a
// second args array.
const PREPARED_STATEMENTS: Record<string, { name: string; param_types: string; sql: string }> = {
  ele: { name: "pxl_ele", param_types: "float8, float8, float8, float8, float8", sql: ESA_ELE_SQL },
  buildings: { name: "pxl_buildings", param_types: "float8, float8, float8, float8", sql: OVERTURE_BUILDINGS_SQL },
  segments: { name: "pxl_segments", param_types: "float8, float8, float8, float8", sql: OVERTURE_TRANSPORTATION_SQL },
  base: { name: "pxl_base", param_types: "float8, float8, float8, float8", sql: OVERTURE_BASE_SQL },
  coast: { name: "pxl_coast", param_types: "float8, float8, float8, float8", sql: OVERTURE_COAST_SQL },
};

/** A JS number as a Postgres float8 literal — safe here because every caller
 * passes bbox coordinates or a scale factor computed from our own page
 * math, never a raw string. Guards against NaN/Infinity reaching raw SQL
 * text regardless, since those would never come from a real page bbox. */
function lit(n: number): string {
  if (!Number.isFinite(n)) throw new Error(`non-finite value in EXECUTE args: ${n}`);
  return n.toString();
}

/** PREPARE is session-scoped, and a pool cycles a fixed set of physical
 * connections through many checkouts — so this runs once per real
 * connection (guarded against "already exists" on every checkout after
 * the first) rather than once per page. */
async function ensure_prepared(client: PoolClient, stmt: { name: string; param_types: string; sql: string }): Promise<void> {
  try {
    await client.queryArray(`PREPARE ${stmt.name} (${stmt.param_types}) AS ${stmt.sql}`);
  } catch (err) {
    const code = (err as { fields?: { code?: string } }).fields?.code;
    if (code !== "42P05") throw err; // 42P05 = duplicate_prepared_statement
  }
}

async function execute<T>(client: PoolClient, key: keyof typeof PREPARED_STATEMENTS, args: number[]): Promise<T[]> {
  const stmt = PREPARED_STATEMENTS[key];
  await ensure_prepared(client, stmt);
  const res = await client.queryObject<T>(`EXECUTE ${stmt.name}(${args.map(lit).join(",")})`);
  return res.rows;
}

export type PageSource = {
  ele: i16[] | null;
  building_rows: BuildingRow[];
  segment_rows: SegmentRow[];
  base_rows: BaseRow[];
  coast_rows: CoastRow[];
};

export class TerrainRepository {
  constructor(private readonly geo_pool: Pool) {}

  /**
   * All five per-page queries (elevation + 4 feature sets), on one checked-out
   * client, run sequentially rather than each self-managing its own
   * pool.connect()/release() — a page only ever holds one connection, so
   * page-level concurrency and pool size mean the same thing (this alone
   * didn't fix the actual slowdown, see PREPARED_STATEMENTS above for that;
   * kept anyway since 5 connections for 1 page's worth of work was never
   * correct regardless).
   */
  async fetch_page_source(page_bbox: BBox3857, page_z_scale: f64): Promise<PageSource> {
    const bbox_args = [page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]];
    const z_cell_per_m: f64 = XY_CELL_PER_M * page_z_scale;

    const client = await this.geo_pool.connect();
    try {
      const ele_rows = await execute<{ ele: i16[] | null }>(client, "ele", [...bbox_args, z_cell_per_m]);
      const building_rows = await execute<BuildingRow>(client, "buildings", bbox_args);
      const segment_rows = await execute<SegmentRow>(client, "segments", bbox_args);
      const base_rows = await execute<BaseRow>(client, "base", bbox_args);
      const coast_rows = await execute<CoastRow>(client, "coast", bbox_args);

      return {
        ele: ele_rows[0]?.ele, // i16[] (row-major) -> i16 LE bytes
        building_rows,
        segment_rows,
        base_rows,
        coast_rows,
      };
    } finally {
      client.release();
    }
  }
}
