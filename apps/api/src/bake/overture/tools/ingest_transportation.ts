// @pxl/api/bake/overture/tools/ingest_transportation.ts
//
// Pulls Overture's Transportation theme (`segment` type only — `connector`,
// pure routing-topology points, skipped: this pipeline only ever rasterizes
// real geometry, never routes anything) for Korea into
// overture_transportation, terrain's own Overture-sourced replacement for
// the road/rail half of osm_elements.
//
// Confirmed live against a real Gangnam-area sample: unlike Buildings,
// `subtype`/`class` are 100% populated here (subtype: road/rail; class:
// residential/service/footway/primary/secondary/steps/tertiary/path/
// subway/cycleway/standard_gauge/...) — this theme's own Korea coverage is
// solid, no thin-data caveat needed the way Buildings has one. `road`
// (structured lane/width data, when present) is kept as JSONB rather than
// flattened — overture_policy.ts (terrain/pipeline/) decides how much of it
// to use; a raw passthrough here means that mapping can change without a
// re-ingest.
//
// Same ingest_one()-per-table shape as ingest_divisions.ts — see that
// file's own top comment for the shared "why not ogr2ogr/infra-geo" and
// "why geojsonseq" reasoning.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_transportation.ts [flags]
//
// Flags:
//   --bbox <minLon,minLat,maxLon,maxLat>  WGS84 box to pull (default: all of South Korea)
//   --out <dir>                           scratch geojsonseq directory (default ./out/overture)

import { Pool, type PoolClient } from "@db/postgres";

import * as env from "@/lib/env/mod.ts";

import { get_overture_release, record_overture_catalog } from "../catalog.ts";

const KR_BBOX_DEFAULT = "124,33,132,39";
const BATCH_SIZE = 300;

const SCRIPT_DIR = new URL(".", import.meta.url).pathname;
const VENV_DIR = `${SCRIPT_DIR}.venv`;
const VENV_OVERTUREMAPS = `${VENV_DIR}/bin/overturemaps`;

type Args = { bbox: string; out: string };

function parse_args(argv: string[]): Args {
  const args: Args = { bbox: KR_BBOX_DEFAULT, out: "./out/overture" };
  for (let i = 0; i < argv.length; i++) {
    switch (argv[i]) {
      case "--bbox":
        args.bbox = argv[++i];
        break;
      case "--out":
        args.out = argv[++i];
        break;
      default:
        throw new Error(`unknown flag: ${argv[i]}`);
    }
  }
  return args;
}

async function path_exists(path: string): Promise<boolean> {
  try {
    await Deno.stat(path);
    return true;
  } catch {
    return false;
  }
}

async function run(cmd: string, args: string[]): Promise<void> {
  const p = new Deno.Command(cmd, { args, stdout: "inherit", stderr: "inherit" });
  const { success, code } = await p.output();
  if (!success) throw new Error(`${cmd} ${args.join(" ")} exited ${code}`);
}

async function ensure_overturemaps_cli(): Promise<void> {
  if (await path_exists(VENV_OVERTUREMAPS)) return;
  console.log(`[bake/overture] no venv at ${VENV_DIR}, creating one and installing overturemaps...`);
  await run("python3", ["-m", "venv", VENV_DIR]);
  await run(`${VENV_DIR}/bin/pip`, ["install", "--quiet", "overturemaps"]);
}

async function* read_lines(path: string): AsyncGenerator<string> {
  const file = await Deno.open(path, { read: true });
  const reader = file.readable.pipeThrough(new TextDecoderStream());
  let buf = "";
  for await (const chunk of reader) {
    buf += chunk;
    let idx: number;
    while ((idx = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, idx);
      buf = buf.slice(idx + 1);
      if (line.length > 0) yield line;
    }
  }
  if (buf.length > 0) yield buf;
}

const SCHEMA_SQL = `
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS overture_transportation;
CREATE TABLE overture_transportation (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT NOT NULL,
  class TEXT,
  road JSONB,
  geom GEOMETRY(LineString, 3857) NOT NULL
);
CREATE INDEX idx_overture_transportation_geom ON overture_transportation USING GIST (geom);
CREATE INDEX idx_overture_transportation_class ON overture_transportation (subtype, class);
`;

type OvertureSegmentFeature = {
  id: string;
  properties: { subtype?: string; class?: string; road?: unknown };
  geometry: unknown;
};

function build_batch_insert(batch: OvertureSegmentFeature[]): { text: string; args: unknown[] } {
  const values: string[] = [];
  const args: unknown[] = [];
  for (const f of batch) {
    const base = args.length;
    args.push(
      f.id,
      f.properties.subtype,
      f.properties.class ?? null,
      f.properties.road !== undefined ? JSON.stringify(f.properties.road) : null,
      JSON.stringify(f.geometry),
    );
    values.push(
      `($${base + 1}, $${base + 2}, $${base + 3}, $${base + 4}, ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON($${base + 5}), 4326), 3857))`,
    );
  }
  return {
    text: `INSERT INTO overture_transportation (gers_id, subtype, class, road, geom) VALUES ${values.join(",")} ON CONFLICT (gers_id) DO NOTHING`,
    args,
  };
}

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake/overture] bbox=${args.bbox} out=${args.out}`);

  await ensure_overturemaps_cli();
  await Deno.mkdir(args.out, { recursive: true });

  const geojsonseq_path = `${args.out}/segments.geojsonseq`;
  console.log("[bake/overture] downloading Transportation/segment extract from Overture's public S3 release...");
  await run(VENV_OVERTUREMAPS, ["download", `--bbox=${args.bbox}`, "-f", "geojsonseq", "--type=segment", "-o", geojsonseq_path]);

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, 4, true);
  try {
    const client: PoolClient = await pool.connect();
    try {
      console.log("[bake/overture] resetting overture_transportation...");
      await client.queryArray(SCHEMA_SQL);

      let batch: OvertureSegmentFeature[] = [];
      let total = 0, skipped = 0;

      const flush = async (): Promise<void> => {
        if (batch.length === 0) return;
        const { text, args: qargs } = build_batch_insert(batch);
        await client.queryArray({ text, args: qargs });
        total += batch.length;
        batch = [];
      };

      for await (const line of read_lines(geojsonseq_path)) {
        const f = JSON.parse(line) as OvertureSegmentFeature;
        // subtype is the one field this table treats as required — every
        // real segment carries road|rail (confirmed 100% live), so its
        // absence means a malformed row, not sparse Korea data.
        if (!f.properties.subtype) {
          skipped++;
          continue;
        }
        batch.push(f);
        if (batch.length >= BATCH_SIZE) await flush();
      }
      await flush();

      console.log("[bake/overture] ANALYZE...");
      await client.queryArray("ANALYZE overture_transportation");

      const { rows } = await client.queryObject<{ count: string }>("SELECT count(*) FROM overture_transportation");
      const total_count = Number(rows[0].count);
      console.log(
        `[bake/overture] done — overture_transportation has ${total_count.toLocaleString()} rows ` +
          `(${total.toLocaleString()} attempted, ${skipped.toLocaleString()} skipped)`,
      );

      const release = await get_overture_release(VENV_OVERTUREMAPS);
      await record_overture_catalog(client, { file_name: "overture_transportation", region_code: args.bbox, data_date: release, total_count });
      console.log(`[bake/overture] catalog: overture_transportation @ release ${release}`);
    } finally {
      client.release();
    }
  } finally {
    await pool.end?.();
  }
}

await main();
