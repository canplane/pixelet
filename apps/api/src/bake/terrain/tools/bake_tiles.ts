// @pxl/api/bake/terrain/tools/bake_tiles.ts
//
// Offline pre-bake pass: walks every L3 page inside the geo DB's coverage
// bbox, skips the ones that are pure open ocean, and bakes the rest into
// gzipped tile objects ready to upload to a static object store (R2) and be
// served straight to the client with no API server in the loop.
//
// Land/ocean classification is a coarse pre-filter, cheaper and separate
// from the live bake path's own fine-grained coastline carving (repository.
// ts's OVERTURE_COAST_SQL, computed from Overture's ocean data). This still
// checks osm_coastline_polygons directly (`&&` + ST_Intersects) — kept
// around specifically for this "does the page touch any land at all"
// existence test, which a cheap bbox-intersects check answers correctly
// either way; a page with zero touching land rows is open ocean and gets
// served by the client's single shared fallback tile instead.
//
// Tile bytes come from ../source.ts, the same assembly the live endpoint
// uses, so a CDN tile and a live-API tile are identical byte for byte.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/terrain/tools/bake_tiles.ts [flags]
//
// Flags:
//   --out <dir>          output directory (default ./out/terrain/v1)
//   --concurrency <n>    parallel bakes (default 8, match DB_POOL_SIZE)
//   --limit <n>          stop after n land tiles (smoke test)
//   --force              re-bake tiles that already exist on disk
//   --reclassify         ignore a cached land_pages.json and re-run the scan

import { Pool } from "@db/postgres";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Bytes } from "@xpute/core/kit/array.ts";
import { encoder, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { PVF3 } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { L3_IDX } from "@pxl/shared/spec/space/indexer.ts";
import * as epsg4326 from "@pxl/shared/spec/space/adapter/4326.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import { ELE_PADDED_SPAN } from "@pxl/shared/bake/constants.ts";
import { bake_terrain } from "@pxl/api/bake/terrain/bake.ts";

import { TerrainRepository } from "../repository.ts";
import { fetch_tile, source_stats } from "../source.ts";
import { exists, fmt_bytes, fmt_dur, get_git_sha, gzip, read_source_catalogs } from "@pxl/api/bake/bake_util.ts";
import * as env from "@/lib/env/mod.ts";

// Current esa_elevation_raster coverage bbox — confirmed via
// `SELECT ST_Extent(ST_Transform(rast::geometry, 4326)) FROM esa_elevation_raster`.
const SW: [lng: number, lat: number] = [123.9998611, 33.000244913560735];
const NE: [lng: number, lat: number] = [131.9997935265517, 39.000138899999996];

const CLASSIFY_BATCH = 2000;

/** Object key of the single fallback tile every open-ocean page resolves to. */
const OCEAN_KEY = "_ocean";

const HAS_LAND_SQL = `
  SELECT t.ord, EXISTS (
    SELECT 1 FROM osm_coastline_polygons c
    WHERE c.geom && ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857)
      AND ST_Intersects(c.geom, ST_MakeEnvelope(t.minx, t.miny, t.maxx, t.maxy, 3857))
  ) AS has_land
  FROM unnest($1::int[], $2::float8[], $3::float8[], $4::float8[], $5::float8[])
    AS t(ord, minx, miny, maxx, maxy)
`;

// ------------------------------------------------------------------------------
// args
// ------------------------------------------------------------------------------

type Args = {
  out: string;
  concurrency: u32;
  limit: number | null;
  force: boolean;
  reclassify: boolean;
};

function parse_args(argv: string[]): Args {
  const args: Args = { out: "./out/terrain/v1", concurrency: 8, limit: null, force: false, reclassify: false };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--out":
        args.out = argv[++i];
        break;
      case "--concurrency":
        args.concurrency = Number(argv[++i]);
        break;
      case "--limit":
        args.limit = Number(argv[++i]);
        break;
      case "--force":
        args.force = true;
        break;
      case "--reclassify":
        args.reclassify = true;
        break;
      default:
        throw new Error(`unknown flag: ${a}`);
    }
  }
  return args;
}

// ------------------------------------------------------------------------------
// helpers
// ------------------------------------------------------------------------------

function project_page(lng: number, lat: number): IVec {
  const pvf3: PVF3 = { i: 0n, f: 0n };
  epsg4326.project(pvf3, [lng, lat, 0]);
  return L3_IDX(pvf3.i);
}

/** Counts and sizes every finished object in the output directory, ignoring
 * `.part` files left behind by an interrupted write. */
async function measure_output(dir: string): Promise<{ count: u32; bytes: number }> {
  let count = 0, bytes = 0;
  for await (const entry of Deno.readDir(dir)) {
    if (!entry.isFile || entry.name.endsWith(".part")) continue;
    const stat = await Deno.stat(`${dir}/${entry.name}`);
    count++, bytes += stat.size;
  }
  return { count, bytes };
}

function same_bytes(a: Bytes, b: Bytes): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

// ------------------------------------------------------------------------------
// candidate grid + classification
// ------------------------------------------------------------------------------

type Candidate = { page_idx: IVec; minx: number; miny: number; maxx: number; maxy: number };

function build_candidates(): Candidate[] {
  const page_sw = project_page(SW[0], SW[1]);
  const page_ne = project_page(NE[0], NE[1]);

  const x1 = ivec.X(page_sw), x2 = ivec.X(page_ne);
  const y1 = ivec.Y(page_sw), y2 = ivec.Y(page_ne);
  const px_min = x1 < x2 ? x1 : x2, px_max = x1 < x2 ? x2 : x1;
  const py_min = y1 < y2 ? y1 : y2, py_max = y1 < y2 ? y2 : y1;

  const out: Candidate[] = [];
  for (let px = px_min; px <= px_max; px++) {
    for (let py = py_min; py <= py_max; py++) {
      const page_idx = ivec.gather(px, py, 0n);
      const bbox = epsg3857.get_page_bbox(page_idx);
      out.push({ page_idx, minx: bbox[0][0], miny: bbox[0][1], maxx: bbox[1][0], maxy: bbox[1][1] });
    }
  }
  console.log(`[bake] page grid x[${px_min}..${px_max}] y[${py_min}..${py_max}] = ${out.length.toLocaleString()} candidate pages`);
  return out;
}

async function classify_land_pages(pool: Pool, candidates: Candidate[]): Promise<IVec[]> {
  const land: IVec[] = [];

  for (let i = 0; i < candidates.length; i += CLASSIFY_BATCH) {
    const batch = candidates.slice(i, i + CLASSIFY_BATCH);

    const client = await pool.connect();
    try {
      const res = await client.queryObject<{ ord: number; has_land: boolean }>({
        text: HAS_LAND_SQL,
        args: [
          batch.map((_, j) => j),
          batch.map((c) => c.minx),
          batch.map((c) => c.miny),
          batch.map((c) => c.maxx),
          batch.map((c) => c.maxy),
        ],
      });
      for (const row of res.rows) {
        if (row.has_land) land.push(batch[row.ord].page_idx);
      }
    } finally {
      client.release();
    }

    const done = Math.min(i + CLASSIFY_BATCH, candidates.length);
    console.log(`[bake/classify] ${done.toLocaleString()}/${candidates.length.toLocaleString()} land=${land.length.toLocaleString()}`);
  }
  return land;
}

// ------------------------------------------------------------------------------
// ocean fallback tile
// ------------------------------------------------------------------------------

/**
 * The source packet an open-ocean page would produce: a flat zero elevation
 * grid, no OSM features, no coastline rows. Synthesised rather than read
 * from the DB so the fallback tile is exactly "nothing here but water",
 * independent of whichever far-offshore page happened to get sampled.
 */
function empty_source(): Bytes {
  const ele = new Int16Array(ELE_PADDED_SPAN * ELE_PADDED_SPAN);
  const empty = new TreeView().branch(() => {});
  return encoder.encode(new TreeView().branch((__) =>
    __.i16_array(ele)
      .subtree(empty) // buildings
      .subtree(empty) // segments
      .subtree(empty) // base
      .subtree(empty) // coasts
  ));
}

/**
 * Bakes the ocean fallback and asserts it really is page-independent — the
 * client reuses one decoded copy for every ocean page, which is only sound
 * if baking the same empty source at different pages (different bbox,
 * different Z_SCALE by latitude) yields identical bytes.
 */
function bake_ocean_tile(samples: IVec[]): Bytes {
  const src = empty_source();
  const baked = samples.map((p) => bake_terrain(p, src));

  for (let i = 1; i < baked.length; i++) {
    if (!same_bytes(baked[0], baked[i])) {
      throw new Error(
        `[bake/ocean] ocean tile is NOT page-independent: page ${ivec.repr(samples[0])} and ${ivec.repr(samples[i])} baked to different bytes ` +
          `(${baked[0].length} vs ${baked[i].length}). The shared-fallback plan doesn't hold; ocean pages would each need their own tile.`,
      );
    }
  }
  return baked[0];
}

// ------------------------------------------------------------------------------
// main
// ------------------------------------------------------------------------------

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake] out=${args.out} concurrency=${args.concurrency} limit=${args.limit ?? "none"} force=${args.force}`);

  await Deno.mkdir(args.out, { recursive: true });

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, args.concurrency + 2, true);
  const repo = new TerrainRepository(pool);

  try {
    const candidates = build_candidates();

    // ---- classify (cached: the scan is deterministic and slow-ish) ----
    const land_cache = `${args.out}/../land_pages.json`;
    let land: IVec[];
    if (!args.reclassify && await exists(land_cache)) {
      const raw = JSON.parse(await Deno.readTextFile(land_cache)) as string[];
      land = raw.map((s) => BigInt(s));
      console.log(`[bake/classify] reusing cached list: ${land.length.toLocaleString()} land pages (${land_cache})`);
    } else {
      land = await classify_land_pages(pool, candidates);
      await Deno.writeTextFile(land_cache, JSON.stringify(land.map((p) => p.toString())));
      console.log(`[bake/classify] done: land=${land.length.toLocaleString()} ocean=${(candidates.length - land.length).toLocaleString()} -> ${land_cache}`);
    }

    // ---- ocean fallback tile ----
    const ocean_path = `${args.out}/${OCEAN_KEY}`;
    if (args.force || !await exists(ocean_path)) {
      const ocean_raw = bake_ocean_tile([land[0], candidates[0].page_idx, candidates[candidates.length - 1].page_idx]);
      const ocean_gz = await gzip(ocean_raw);
      await Deno.writeFile(ocean_path, ocean_gz);
      console.log(`[bake/ocean] wrote ${OCEAN_KEY}: raw=${fmt_bytes(ocean_raw.length)} gz=${fmt_bytes(ocean_gz.length)} (page-independent: verified)`);
    }

    // ---- bake land tiles ----
    const targets = args.limit === null ? land : land.slice(0, args.limit);

    let next = 0;
    let baked = 0, skipped = 0, failed = 0;
    let raw_total = 0, gz_total = 0;
    const started = performance.now();

    const worker = async (): Promise<void> => {
      for (;;) {
        const i = next++;
        if (i >= targets.length) return;

        const page_idx = targets[i];
        const path = `${args.out}/${page_idx.toString()}`;

        if (!args.force && await exists(path)) {
          skipped++;
          continue;
        }

        try {
          const raw = await fetch_tile(repo, page_idx);
          const gz = await gzip(raw);
          // write to a temp name then rename: a crash mid-write would
          // otherwise leave a truncated file that the resume path (exists()
          // -> skip) treats as a finished tile forever after.
          await Deno.writeFile(`${path}.part`, gz);
          await Deno.rename(`${path}.part`, path);

          baked++;
          raw_total += raw.length, gz_total += gz.length;
        } catch (err) {
          failed++;
          console.error(`[bake/tile/${ivec.repr(page_idx)}] failed:`, err);
        }

        const done = baked + skipped + failed;
        if (done % 200 === 0) {
          const elapsed = performance.now() - started;
          const rate = baked > 0 ? elapsed / baked : 0;
          const eta = rate * (targets.length - done);
          console.log(
            `[bake] ${done.toLocaleString()}/${targets.length.toLocaleString()} baked=${baked.toLocaleString()} skipped=${skipped.toLocaleString()} failed=${failed} ` +
              `gz=${fmt_bytes(gz_total)} ${rate.toFixed(0)}ms/tile eta=${fmt_dur(eta)}`,
          );
        }
      }
    };

    await Promise.all(Array.from({ length: args.concurrency }, () => worker()));

    const elapsed = performance.now() - started;
    const avg_gz = baked > 0 ? gz_total / baked : 0;
    const projected = avg_gz * land.length;

    console.log("");
    console.log(`[bake] done in ${fmt_dur(elapsed)}: baked=${baked.toLocaleString()} skipped=${skipped.toLocaleString()} failed=${failed}`);
    console.log(`[bake] raw=${fmt_bytes(raw_total)} gz=${fmt_bytes(gz_total)} (avg ${fmt_bytes(avg_gz)}/tile gzipped, ${(gz_total / Math.max(raw_total, 1) * 100).toFixed(1)}% of raw)`);
    console.log(`[bake] projected full set (${land.length.toLocaleString()} land tiles): ${fmt_bytes(projected)}`);

    // A degraded tile still writes successfully, so it never shows up in
    // `failed` — surface it here rather than leaving it buried in per-page
    // warnings that nobody greps for after a 40-minute run.
    if (source_stats.invalid_ele_shape > 0) {
      const pct = (source_stats.invalid_ele_shape / Math.max(baked, 1)) * 100;
      console.warn(
        `[bake] WARNING: ${source_stats.invalid_ele_shape.toLocaleString()} of ${baked.toLocaleString()} tiles (${pct.toFixed(2)}%) had an invalid elevation shape ` +
          `and were baked FLAT (zero-filled elevation). Expected at the outer edge of the DEM coverage, where the padded sampling window falls outside the source raster.`,
      );
    }

    // The run counters above describe one invocation, and this tool resumes —
    // a re-run that skips everything would otherwise write a manifest claiming
    // the tile set is empty. Measure what is actually on disk instead, so the
    // manifest describes the artifact rather than the invocation that last
    // touched it.
    const on_disk = await measure_output(args.out);

    const manifest = {
      version: args.out.split("/").filter(Boolean).pop(),
      updated_at: new Date().toISOString(),
      // the catalog below says which *data* this was baked from; this says
      // which *code* did the baking (bake_terrain/ground_baker/normalizer
      // are all pure functions of these two things together).
      git_sha: await get_git_sha(),
      bbox_4326: { sw: SW, ne: NE },

      // the published tile set, measured from disk
      objects: on_disk.count,
      gz_bytes: on_disk.bytes,
      candidate_pages: candidates.length,
      land_pages: land.length,
      ocean_pages: candidates.length - land.length,
      ocean_key: OCEAN_KEY,

      // every object is stored pre-gzipped: upload with
      //   content-type: application/octet-stream
      //   content-encoding: gzip
      // so fetch() on the client decompresses transparently.
      content_type: "application/octet-stream",
      content_encoding: "gzip",

      // this invocation only — a resumed run reports only what it re-baked,
      // so a zero here means "nothing to do", not "nothing wrong".
      run: {
        baked: baked,
        skipped: skipped,
        failed: failed,
        gz_bytes: gz_total,
        flat_elevation_tiles: source_stats.invalid_ele_shape,
      },

      source: await read_source_catalogs(pool),
    };
    await Deno.writeTextFile(`${args.out}/../manifest.json`, JSON.stringify(manifest, null, 2));
    console.log(`[bake] manifest -> ${args.out}/../manifest.json`);
  } finally {
    await pool.end?.();
  }
}

await main();
