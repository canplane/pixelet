// @pxl/api/bake/geocode/tools/bake_geocode_tiles.ts
//
// Offline pre-bake pass for reverse geocoding — same shape as
// bake/poi/tools/bake_poi_tiles.ts: reuses terrain's own land-page
// classification (land_pages.json), asks the current GeocodeProvider
// (providers/overture_provider.ts) for each page's neighborhood label, and
// writes one small gzip JSON object per non-empty page. A page whose
// center falls outside every ingested division polygon (open water, or
// simply outside Overture's Divisions coverage) writes a local `.empty`
// marker, never uploaded, same as POI's own convention.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/geocode/tools/bake_geocode_tiles.ts [flags]
//
// Flags:
//   --out <dir>          output directory (default ./out/geocode/v1)
//   --land-cache <path>  terrain's own land_pages.json (default ./out/tiles/land_pages.json)
//   --concurrency <n>    parallel fetches (default 8)
//   --limit <n>          stop after n land pages (smoke test)
//   --force              re-fetch pages that already have an object or .empty marker

import { Pool } from "@db/postgres";

import type { u32 } from "@xpute/core/kit/word.ts";
import type { Bytes } from "@xpute/core/kit/array.ts";

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { exists, fmt_bytes, fmt_dur, gzip } from "@pxl/api/bake/bake_util.ts";

import { make_overture_geocode_provider } from "@pxl/api/modules/world/geocode/providers/overture_provider.ts";
import type { GeocodeProvider } from "@pxl/api/modules/world/geocode/provider.ts";
import * as env from "@/lib/env/mod.ts";

const EMPTY_SUFFIX = ".empty";

type Args = { out: string; land_cache: string; concurrency: u32; limit: number | null; force: boolean };

function parse_args(argv: string[]): Args {
  const args: Args = { out: "./out/geocode/v1", land_cache: "./out/tiles/land_pages.json", concurrency: 8, limit: null, force: false };
  for (let i = 0; i < argv.length; i++) {
    switch (argv[i]) {
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
        throw new Error(`unknown flag: ${argv[i]}`);
    }
  }
  return args;
}

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake/geocode] out=${args.out} land_cache=${args.land_cache} concurrency=${args.concurrency} limit=${args.limit ?? "none"} force=${args.force}`);

  if (!await exists(args.land_cache)) {
    throw new Error(`[bake/geocode] ${args.land_cache} not found — run terrain/tools/bake_tiles.ts's own classification first.`);
  }
  await Deno.mkdir(args.out, { recursive: true });

  const raw = JSON.parse(await Deno.readTextFile(args.land_cache)) as string[];
  const land: IVec[] = raw.map((s) => BigInt(s));
  console.log(`[bake/geocode] ${land.length.toLocaleString()} land pages from ${args.land_cache}`);

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, args.concurrency + 2, true);
  const provider: GeocodeProvider = make_overture_geocode_provider(pool);

  try {
    const targets = args.limit === null ? land : land.slice(0, args.limit);

    let next = 0;
    let baked = 0, empty = 0, skipped = 0, failed = 0, gz_total = 0;
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
          const result = await provider.reverse(page_idx);

          if (result === null) {
            await Deno.writeTextFile(`${path}${EMPTY_SUFFIX}`, "");
            empty++;
          } else {
            const raw_json: Bytes = new TextEncoder().encode(JSON.stringify(result));
            const gz = await gzip(raw_json);
            await Deno.writeFile(`${path}.part`, gz);
            await Deno.rename(`${path}.part`, path);

            baked++;
            gz_total += gz.length;
          }
        } catch (err) {
          failed++;
          console.error(`[bake/geocode/${ivec.repr(page_idx)}] failed:`, err);
        }

        const done = baked + empty + skipped + failed;
        if (done % 2000 === 0) {
          const elapsed = performance.now() - started;
          const rate = done > skipped ? elapsed / (done - skipped) : 0;
          const eta = rate * (targets.length - done);
          console.log(
            `[bake/geocode] ${done.toLocaleString()}/${targets.length.toLocaleString()} baked=${baked.toLocaleString()} empty=${empty.toLocaleString()} ` +
              `skipped=${skipped.toLocaleString()} failed=${failed} gz=${fmt_bytes(gz_total)} ${rate.toFixed(0)}ms/page eta=${fmt_dur(eta)}`,
          );
        }
      }
    };

    await Promise.all(Array.from({ length: args.concurrency }, () => worker()));

    const elapsed = performance.now() - started;
    console.log("");
    console.log(`[bake/geocode] done in ${fmt_dur(elapsed)}: baked=${baked.toLocaleString()} empty=${empty.toLocaleString()} skipped=${skipped.toLocaleString()} failed=${failed}`);
    console.log(`[bake/geocode] gz=${fmt_bytes(gz_total)}`);

    const manifest = {
      version: args.out.split("/").filter(Boolean).pop(),
      updated_at: new Date().toISOString(),
      land_pages: land.length,
      run: { baked, empty, skipped, failed, gz_bytes: gz_total },
      content_type: "application/json",
      content_encoding: "gzip",
      source: "overture_divisions + overture_division_points (region, county, locality, neighborhood, microhood, macrohood)",
    };
    await Deno.writeTextFile(`${args.out}/../manifest.json`, JSON.stringify(manifest, null, 2));
    console.log(`[bake/geocode] manifest -> ${args.out}/../manifest.json`);
  } finally {
    await pool.end?.();
  }
}

await main();
