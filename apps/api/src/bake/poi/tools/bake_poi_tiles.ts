// @pxl/api/bake/poi/tools/bake_poi_tiles.ts
//
// Offline pre-bake pass: reads terrain's own land-page classification
// (terrain/tools/bake_tiles.ts's own land_pages.json — no separate POI
// classification pass, see below) and, for each one, asks the current
// PoiProvider (providers/overture_provider.ts today) for that page's POIs. A
// page with none writes a local `.empty` marker instead of an object, so a
// resumed run can skip it without re-querying the DB — that marker is
// never uploaded (see "Publishing" below).
//
// Reusing terrain's land/ocean split rather than a POI-specific
// classification is deliberate: "does this bbox touch any land polygon"
// (terrain's own predicate) is a cheap, purely geometric pre-filter that
// already rules out every page this tool could never produce POIs for
// (pure ocean); a real "does this page have any POI-taggable feature"
// classification would cost about as much as just fetching and extracting,
// so there's nothing cheaper to pre-filter on beyond what terrain already
// computed. Land pages with zero named places still cost one DB round trip
// each; that's the `.empty` marker's own job to make a one-time cost, not
// eliminate.
//
// No shared "everything's the same" fallback exists here the way terrain's
// `_ocean` tile does: "no POI here" is just an empty result, not a real
// payload some other page's bytes could stand in for, so a missing key is
// enough on its own — the client can treat 404 as `[]` directly.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/poi/tools/bake_poi_tiles.ts [flags]
//
// Flags:
//   --out <dir>          output directory (default ./out/poi/v1)
//   --land-cache <path>  terrain's own land_pages.json (default ./out/tiles/land_pages.json)
//   --concurrency <n>    parallel fetches (default 8, match DB_POOL_SIZE)
//   --limit <n>          stop after n land pages (smoke test)
//   --force              re-fetch pages that already have an object or .empty marker

import { Pool } from "@db/postgres";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Bytes } from "@xpute/core/kit/array.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";
import { exists, fmt_bytes, fmt_dur, gzip } from "@pxl/api/bake/bake_util.ts";

import { make_overture_provider } from "@pxl/api/modules/world/poi/providers/overture_provider.ts";
import type { PoiProvider } from "@pxl/api/modules/world/poi/provider.ts";
import * as env from "@/lib/env/mod.ts";

const EMPTY_SUFFIX = ".empty";

// ------------------------------------------------------------------------------
// args
// ------------------------------------------------------------------------------

type Args = {
  out: string;
  land_cache: string;
  concurrency: u32;
  limit: number | null;
  force: boolean;
};

function parse_args(argv: string[]): Args {
  const args: Args = {
    out: "./out/poi/v1",
    land_cache: "./out/tiles/land_pages.json",
    concurrency: 8,
    limit: null,
    force: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    switch (a) {
      case "--out":
        args.out = argv[++i];
        break;
      case "--land-cache":
        args.land_cache = argv[++i];
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
      default:
        throw new Error(`unknown flag: ${a}`);
    }
  }
  return args;
}

// ------------------------------------------------------------------------------
// main
// ------------------------------------------------------------------------------

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake/poi] out=${args.out} land_cache=${args.land_cache} concurrency=${args.concurrency} limit=${args.limit ?? "none"} force=${args.force}`);

  if (!await exists(args.land_cache)) {
    throw new Error(
      `[bake/poi] ${args.land_cache} not found — run terrain/tools/bake_tiles.ts's own classification first ` +
        `(it writes this as a side effect; --limit 0 skips baking tiles if only the classification is wanted).`,
    );
  }
  await Deno.mkdir(args.out, { recursive: true });

  const raw = JSON.parse(await Deno.readTextFile(args.land_cache)) as string[];
  const land: IVec[] = raw.map((s) => BigInt(s));
  console.log(`[bake/poi] ${land.length.toLocaleString()} land pages from ${args.land_cache}`);

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, args.concurrency + 2, true);
  const provider: PoiProvider = make_overture_provider(pool);

  try {
    const targets = args.limit === null ? land : land.slice(0, args.limit);

    let next = 0;
    let baked = 0, empty = 0, skipped = 0, failed = 0;
    let poi_total = 0, gz_total = 0;
    const started = performance.now();

    const worker = async (): Promise<void> => {
      for (;;) {
        const i = next++;
        if (i >= targets.length) return;

        const page_idx = targets[i];
        const path = `${args.out}/${page_idx.toString()}`;

        if (!args.force && (await exists(path) || await exists(`${path}${EMPTY_SUFFIX}`))) {
          skipped++;
          continue;
        }

        try {
          const pois: POI[] = await provider.fetch_pois(page_idx);

          if (pois.length === 0) {
            // Local-only marker — never uploaded (exclude "*.empty" from
            // the S3 sync). Its only job is telling a resumed run "already
            // checked, nothing here" without re-querying the DB.
            await Deno.writeTextFile(`${path}${EMPTY_SUFFIX}`, "");
            empty++;
          } else {
            const raw_json: Bytes = new TextEncoder().encode(JSON.stringify(pois));
            const gz = await gzip(raw_json);
            // write to a temp name then rename: a crash mid-write would
            // otherwise leave a truncated file the resume path (exists() ->
            // skip) treats as finished forever after.
            await Deno.writeFile(`${path}.part`, gz);
            await Deno.rename(`${path}.part`, path);

            baked++;
            poi_total += pois.length, gz_total += gz.length;
          }
        } catch (err) {
          failed++;
          console.error(`[bake/poi/${ivec.repr(page_idx)}] failed:`, err);
        }

        const done = baked + empty + skipped + failed;
        if (done % 500 === 0) {
          const elapsed = performance.now() - started;
          const rate = done > skipped ? elapsed / (done - skipped) : 0;
          const eta = rate * (targets.length - done);
          console.log(
            `[bake/poi] ${done.toLocaleString()}/${targets.length.toLocaleString()} baked=${baked.toLocaleString()} empty=${empty.toLocaleString()} ` +
              `skipped=${skipped.toLocaleString()} failed=${failed} gz=${fmt_bytes(gz_total)} ${rate.toFixed(0)}ms/page eta=${fmt_dur(eta)}`,
          );
        }
      }
    };

    await Promise.all(Array.from({ length: args.concurrency }, () => worker()));

    const elapsed = performance.now() - started;
    console.log("");
    console.log(`[bake/poi] done in ${fmt_dur(elapsed)}: baked=${baked.toLocaleString()} empty=${empty.toLocaleString()} skipped=${skipped.toLocaleString()} failed=${failed}`);
    console.log(`[bake/poi] ${poi_total.toLocaleString()} POI across ${baked.toLocaleString()} non-empty pages, gz=${fmt_bytes(gz_total)}`);

    const manifest = {
      version: args.out.split("/").filter(Boolean).pop(),
      updated_at: new Date().toISOString(),
      land_pages: land.length,

      // this invocation only, same caveat bake_tiles.ts's own manifest
      // documents — a resumed run only reports what it re-did.
      run: { baked, empty, skipped, failed, poi_total, gz_bytes: gz_total },

      content_type: "application/json",
      content_encoding: "gzip",

      // Not read_source_catalogs()'s OSM/ESA/coastline tables — POI no
      // longer depends on those (discussions/20260908-overture-poi-adoption.md).
      // overture_places has no catalog table of its own yet (see
      // ingest_places.ts's own top comment on why this pipeline stays
      // lighter-weight than infra/geo's OSM ones); this at least records
      // which table and how many rows this bake actually read.
      source: {
        overture_places: await (async () => {
          const client = await pool.connect();
          try {
            const { rows } = await client.queryObject<{ count: string }>("SELECT count(*) FROM overture_places");
            return { row_count: Number(rows[0].count) };
          } finally {
            client.release();
          }
        })(),
      },
    };
    await Deno.writeTextFile(`${args.out}/../manifest.json`, JSON.stringify(manifest, null, 2));
    console.log(`[bake/poi] manifest -> ${args.out}/../manifest.json`);
  } finally {
    await pool.end?.();
  }
}

await main();
