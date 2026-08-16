// @pxl/api/bake/terrain/tools/classify_ocean_pages.ts
//
// One-off analysis tool (not part of the runtime server): for every L3 page
// inside the geo DB's current data coverage bbox, ask whether any land
// polygon (osm_coastline_polygons) touches it. A page with zero touching
// land rows has no land data anywhere nearby and can be safely classified
// as open ocean — a candidate to skip when pre-baking terrain tiles for
// static CDN upload.
//
// This mirrors OSM_COAST_SQL's own `&&` + ST_Intersects filter (see
// repository.ts) rather than inventing a new predicate, so "has_land" here
// means exactly the same thing the live bake path already checks.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/terrain/tools/classify_ocean_pages.ts

import { Pool } from "@db/postgres";

import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { IVec, PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";

import * as env from "@/lib/env/mod.ts";

// Current esa_elevation_raster coverage bbox (see conversation: confirmed via
// `SELECT ST_Extent(ST_Transform(rast::geometry, 4326)) FROM esa_elevation_raster`).
const SW: [lng: number, lat: number] = [123.9998611, 33.000244913560735];
const NE: [lng: number, lat: number] = [131.9997935265517, 39.000138899999996];

const BATCH_SIZE = 2000;

const HAS_LAND_SQL = `
  SELECT t.ord, EXISTS (
    SELECT 1 FROM osm_coastline_polygons c
    WHERE c.geom && ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857)
      AND ST_Intersects(c.geom, ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857))
  ) AS has_land
  FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[])
    AS t(ord, minx, miny, maxx, maxy)
`;

function project_page(lng: number, lat: number): IVec {
  const pvf3: PVF3 = { i: 0n, f: 0n };
  epsg4326.project(pvf3, [lng, lat, 0]);
  return L3_IDX(pvf3.i);
}

async function main(): Promise<void> {
  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, 4, true);

  try {
    const page_sw = project_page(SW[0], SW[1]);
    const page_ne = project_page(NE[0], NE[1]);

    const x1 = ivec.X(page_sw), x2 = ivec.X(page_ne);
    const y1 = ivec.Y(page_sw), y2 = ivec.Y(page_ne);
    const px_min = x1 < x2 ? x1 : x2, px_max = x1 < x2 ? x2 : x1;
    const py_min = y1 < y2 ? y1 : y2, py_max = y1 < y2 ? y2 : y1;

    const total_pages = Number(px_max - px_min + 1n) * Number(py_max - py_min + 1n);
    console.log(`[classify] page grid: x[${px_min}..${px_max}] y[${py_min}..${py_max}] = ${total_pages.toLocaleString()} candidate pages`);

    // build the full candidate list up front (pure math, cheap even at ~120k)
    const candidates: { page_idx: IVec; minx: number; miny: number; maxx: number; maxy: number }[] = [];
    for (let px = px_min; px <= px_max; px++) {
      for (let py = py_min; py <= py_max; py++) {
        const page_idx = ivec.gather(px, py, 0n);
        const bbox = epsg3857.get_page_bbox(page_idx);
        candidates.push({ page_idx, minx: bbox[0][0], miny: bbox[0][1], maxx: bbox[1][0], maxy: bbox[1][1] });
      }
    }

    let land_count = 0;
    let ocean_count = 0;

    for (let i = 0; i < candidates.length; i += BATCH_SIZE) {
      const batch = candidates.slice(i, i + BATCH_SIZE);

      const ord = batch.map((_, j) => j);
      const minx = batch.map((c) => c.minx);
      const miny = batch.map((c) => c.miny);
      const maxx = batch.map((c) => c.maxx);
      const maxy = batch.map((c) => c.maxy);

      const client = await pool.connect();
      try {
        const res = await client.queryObject<{ ord: number; has_land: boolean }>({
          text: HAS_LAND_SQL,
          args: [ord, minx, miny, maxx, maxy],
        });
        for (const row of res.rows) {
          if (row.has_land) land_count++;
          else ocean_count++;
        }
      } finally {
        client.release();
      }

      const done = Math.min(i + BATCH_SIZE, candidates.length);
      console.log(`[classify] ${done.toLocaleString()}/${candidates.length.toLocaleString()} (land=${land_count.toLocaleString()}, ocean=${ocean_count.toLocaleString()})`);
    }

    console.log("");
    console.log(`[classify] done. total=${candidates.length.toLocaleString()} land-or-coastal=${land_count.toLocaleString()} pure-ocean(skip)=${ocean_count.toLocaleString()}`);

    for (const kb of [100, 125, 150]) {
      const gb = (land_count * kb) / (1024 * 1024);
      console.log(`[classify]   at ${kb}kB/tile -> ${gb.toFixed(2)} GB for the ${land_count.toLocaleString()} tiles that need baking`);
    }
  } finally {
    await pool.end?.();
  }
}

await main();
