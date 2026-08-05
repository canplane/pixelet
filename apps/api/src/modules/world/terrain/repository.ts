// @pxl/api/modules/world/terrain/repository.ts

import { Pool } from "@db/postgres";

import type { f64, i16 } from "@xpute/core/kit/word.ts";

import type { CoastRow, FeatRow } from "@pxl/shared/bake/format.ts";
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

const OSM_FEAT_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT
  osm_id,
  tags::text AS tags_hstore,
  -- Keep original geometry; clipping can destroy closed-loop topology.
  ST_AsBinary(way) AS way_wkb,
  ST_AsBinary(ST_PointOnSurface(way)) AS center_wkb
FROM osm_elements, env
-- Fetch every feature whose bbox overlaps the page envelope.
WHERE way && env.e`;

const OSM_COAST_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
),
clipped AS (
  SELECT
    fid AS id,
    -- Coast polygons are global-scale areas; clipping here is intentional.
    ST_Intersection(geom, env.e) AS g,
    env.e AS e
  FROM osm_coastline_polygons, env
  WHERE geom && env.e
    AND ST_Intersects(geom, env.e)
)
SELECT
  id,
  ST_AsBinary(g) AS geom_wkb,
  ST_Covers(g, e) AS covers_page
FROM clipped
WHERE NOT ST_IsEmpty(g)`;

export class TerrainRepository {
  constructor(private readonly geo_pool: Pool) {}

  async fetch_ele(page_bbox: BBox3857, page_z_scale: f64): Promise<i16[] | null> {
    const client = await this.geo_pool.connect();
    try {
      const z_cell_per_m: f64 = XY_CELL_PER_M * page_z_scale;

      const res = await client.queryObject<{ ele: i16[] | null }>({
        text: ESA_ELE_SQL,
        args: [
          page_bbox[0][0], // $1 min x
          page_bbox[0][1], // $2 min y
          page_bbox[1][0], // $3 max x
          page_bbox[1][1], // $4 max y
          z_cell_per_m, // $5
        ],
      });
      return res.rows[0]?.ele; // i16[] (row-major) -> i16 LE bytes
    } finally {
      client.release();
    }
  }

  async fetch_feat_rows(page_bbox: BBox3857): Promise<FeatRow[]> {
    const client = await this.geo_pool.connect();
    try {
      const res = await client.queryObject<FeatRow>({
        text: OSM_FEAT_SQL,
        args: [page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]],
      });
      return res.rows;
    } finally {
      client.release();
    }
  }

  async fetch_coast_rows(page_bbox: BBox3857): Promise<CoastRow[]> {
    const client = await this.geo_pool.connect();
    try {
      const res = await client.queryObject<CoastRow>({
        text: OSM_COAST_SQL,
        args: [page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]],
      });
      return res.rows;
    } finally {
      client.release();
    }
  }
}
