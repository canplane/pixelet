// @pxl/api/modules/world/poi/providers/overture_provider.ts

import type { Pool } from "@db/postgres";

import type { f64 } from "@xpute/core/kit/word.ts";
import type { V3 } from "@pxl/shared/spec/space/v3.ts";
import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { XY_CELL_PER_M } from "@pxl/shared/bake/constants.ts";
import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";

import { OVERTURE_CATEGORY_MAP } from "@pxl/api/bake/overture/model/category_map.ts";
import type { PoiProvider } from "../provider.ts";

const OVERTURE_PLACES_SQL = `WITH env AS (
  SELECT ST_MakeEnvelope($1, $2, $3, $4, 3857) AS e
)
SELECT
  gers_id,
  properties,
  ST_X(geom) AS x, ST_Y(geom) AS y
FROM overture_places, env
-- Fetch every place whose point falls in the page envelope.
WHERE geom && env.e`;

type Row = { gers_id: string; properties: Record<string, unknown>; x: number; y: number };

function name_of(properties: Record<string, unknown>): string {
  const names = properties.names as { primary?: string } | undefined;
  return names?.primary ?? "";
}

function overture_category(properties: Record<string, unknown>): string | undefined {
  return (properties.categories as { primary?: string } | undefined)?.primary;
}

function category_of(properties: Record<string, unknown>): string {
  const primary = overture_category(properties);
  return (primary && OVERTURE_CATEGORY_MAP[primary]) ?? primary ?? "place";
}

/**
 * Overture's Places theme (docs.overturemaps.org/guides/places) —
 * POI's baseline switched here from OSM
 * (discussions/20260908-overture-poi-adoption.md): Places is licensed
 * CDLA-Permissive-2.0, sourced from Meta/Microsoft/Foursquare/PinMeTo, has
 * no OpenStreetMap content and none of the display/caching restrictions
 * Google/Naver/Kakao carry. Reads from `overture_places`
 * (bake/overture/tools/ingest_places.ts's own table — a Korea-bbox extract
 * pulled from Overture's public S3 release, refreshed on whatever cadence
 * that tool is re-run at, not a live per-request call).
 *
 * Deliberately its own table, not merged into osm_elements: Overture's
 * per-place shape is a rich nested JSON object with no flat-tag equivalent
 * in OSM's model, and this pipeline is meant to stay structurally
 * independent of the terrain pipeline POI used to depend on — not layered
 * on top of it (see the discussion doc's own "don't mix in one place" note).
 *
 * `rank` has no OSM-style policy-assigned equivalent here — Overture's own
 * `properties.confidence` (a source-conflation quality score, not a
 * notability score) is the closest available signal, scaled to a plain
 * integer. select_nearby_pois (poi_layer.tsx) ranks by distance to
 * map_center regardless, so this only matters if a future consumer wants
 * an actual notability cut.
 */
export function make_overture_provider(pool: Pool): PoiProvider {
  return {
    async fetch_pois(page_idx: IVec): Promise<POI[]> {
      const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
      const page_min = page_bbox[0];

      const client = await pool.connect();
      let rows: Row[];
      try {
        const res = await client.queryObject<Row>({
          text: OVERTURE_PLACES_SQL,
          args: [page_bbox[0][0], page_bbox[0][1], page_bbox[1][0], page_bbox[1][1]],
        });
        rows = res.rows;
      } finally {
        client.release();
      }

      const pois: POI[] = [];
      for (const row of rows) {
        const name = name_of(row.properties);
        if (name.length === 0) continue; // an unnamed place isn't worth a badge

        const center: V3 = [
          (row.x - page_min[0]) * XY_CELL_PER_M,
          (row.y - page_min[1]) * XY_CELL_PER_M,
          0,
        ];
        const confidence = (row.properties.confidence as f64 | undefined) ?? 0;

        pois.push({
          id: row.gers_id,
          type: "poi",
          category: category_of(row.properties),
          name,
          center,
          tags: { category: overture_category(row.properties) },
          rank: Math.round(confidence * 1000),
        });
      }
      return pois;
    },
  };
}
