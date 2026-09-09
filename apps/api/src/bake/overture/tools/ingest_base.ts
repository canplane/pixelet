// @pxl/api/bake/overture/tools/ingest_base.ts
//
// Pulls Overture's Base theme for Korea into four tables — terrain's own
// Overture-sourced replacement for the land/water/landuse half of
// osm_elements. Base is fragmented across several `overturemaps --type`
// values rather than one, unlike Places/Divisions/Buildings/Transportation:
//
//   overture_base_land_use  (`land_use`, polygons: residential/park/
//                            commercial/school/hospital/pitch/... — this
//                            project's own main land-coloring signal)
//   overture_base_water     (`water`, polygons: river/lake/stream/canal)
//   overture_base_land      (`land`, polygons: tree/forest/wood/peninsula —
//                            Overture's own natural-cover equivalent of
//                            OSM's natural=wood/forest, still needed
//                            separately from land_use since a green patch
//                            with no administrative land_use polygon over
//                            it is common)
//   overture_base_infrastructure (`infrastructure`, mixed geometry — kept
//                            for only parking/helipad classes; confirmed
//                            live the theme is otherwise almost entirely
//                            point-scale street furniture with no ground-
//                            paintable equivalent — crossing/bus_stop/
//                            traffic_signals/gate/fence/bicycle_rental/
//                            recycling/emergency/power all skipped)
//
// `land_cover` (`--type=land_cover`) is deliberately not ingested at all —
// imagery/vegetation-index-derived, not administrative like land_use, and
// this project's own coarse land coloring doesn't need a second, likely-
// redundant vegetation signal on top of land_use + land.
//
// Same generic ingest_one()-per-table shape as ingest_divisions.ts — see
// that file's own top comment for the shared "why not ogr2ogr/infra-geo"
// and "why geojsonseq" reasoning.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_base.ts [flags]
//
// Flags:
//   --bbox <minLon,minLat,maxLon,maxLat>  WGS84 box to pull (default: all of South Korea)
//   --out <dir>                           scratch geojsonseq directory (default ./out/overture)

import { Pool, type PoolClient } from "@db/postgres";

import * as env from "@/lib/env/mod.ts";

import { get_overture_release, record_overture_catalog } from "../catalog.ts";

const KR_BBOX_DEFAULT = "124,33,132,39";
const BATCH_SIZE = 300;

// infrastructure is otherwise near-entirely point-scale street furniture
// (confirmed live: crossing/bus_stop/traffic_signals/gate/fence/
// bicycle_rental/recycling/emergency/power) — no ground-channel equivalent
// exists for any of those, so only classes this pipeline can actually
// paint are kept.
const INFRA_KEPT_CLASSES = new Set(["parking", "helipad"]);

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

function schema_sql(table: string): string {
  return `
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS ${table};
CREATE TABLE ${table} (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT,
  class TEXT,
  geom GEOMETRY(Geometry, 3857) NOT NULL
);
CREATE INDEX idx_${table}_geom ON ${table} USING GIST (geom);
CREATE INDEX idx_${table}_class ON ${table} (class);
`;
}

type OvertureBaseFeature = {
  id: string;
  properties: { subtype?: string; class?: string };
  geometry: unknown;
};

function build_batch_insert(table: string, batch: OvertureBaseFeature[]): { text: string; args: unknown[] } {
  const values: string[] = [];
  const args: unknown[] = [];
  for (const f of batch) {
    const base = args.length;
    args.push(f.id, f.properties.subtype ?? null, f.properties.class ?? null, JSON.stringify(f.geometry));
    values.push(
      `($${base + 1}, $${base + 2}, $${base + 3}, ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON($${base + 4}), 4326), 3857))`,
    );
  }
  return {
    text: `INSERT INTO ${table} (gers_id, subtype, class, geom) VALUES ${values.join(",")} ON CONFLICT (gers_id) DO NOTHING`,
    args,
  };
}

async function ingest_one(
  client: PoolClient,
  overture_type: string,
  geojsonseq_path: string,
  bbox: string,
  table: string,
  kept_classes: Set<string> | null,
): Promise<void> {
  console.log(`[bake/overture] downloading Base/${overture_type} extract from Overture's public S3 release...`);
  await run(VENV_OVERTUREMAPS, ["download", `--bbox=${bbox}`, "-f", "geojsonseq", `--type=${overture_type}`, "-o", geojsonseq_path]);

  console.log(`[bake/overture] resetting ${table}...`);
  await client.queryArray(schema_sql(table));

  let batch: OvertureBaseFeature[] = [];
  let total = 0, skipped = 0;

  const flush = async (): Promise<void> => {
    if (batch.length === 0) return;
    const { text, args } = build_batch_insert(table, batch);
    await client.queryArray({ text, args });
    total += batch.length;
    batch = [];
  };

  for await (const line of read_lines(geojsonseq_path)) {
    const f = JSON.parse(line) as OvertureBaseFeature;
    if (kept_classes && (!f.properties.class || !kept_classes.has(f.properties.class))) {
      skipped++;
      continue;
    }
    batch.push(f);
    if (batch.length >= BATCH_SIZE) await flush();
  }
  await flush();

  console.log(`[bake/overture] ${table}: ${total.toLocaleString()} rows inserted, ${skipped.toLocaleString()} skipped`);
}

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake/overture] bbox=${args.bbox} out=${args.out}`);

  await ensure_overturemaps_cli();
  await Deno.mkdir(args.out, { recursive: true });

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, 4, true);
  try {
    const client = await pool.connect();
    try {
      await ingest_one(client, "land_use", `${args.out}/land_use.geojsonseq`, args.bbox, "overture_base_land_use", null);
      await ingest_one(client, "water", `${args.out}/base_water.geojsonseq`, args.bbox, "overture_base_water", null);
      await ingest_one(client, "land", `${args.out}/land.geojsonseq`, args.bbox, "overture_base_land", null);
      await ingest_one(
        client,
        "infrastructure",
        `${args.out}/infrastructure.geojsonseq`,
        args.bbox,
        "overture_base_infrastructure",
        INFRA_KEPT_CLASSES,
      );

      const tables = ["overture_base_land_use", "overture_base_water", "overture_base_land", "overture_base_infrastructure"];

      console.log("[bake/overture] ANALYZE...");
      for (const table of tables) {
        await client.queryArray(`ANALYZE ${table}`);
      }

      const release = await get_overture_release(VENV_OVERTUREMAPS);
      for (const table of tables) {
        const { rows } = await client.queryObject<{ count: string }>(`SELECT count(*) FROM ${table}`);
        const total_count = Number(rows[0].count);
        console.log(`[bake/overture] ${table} has ${total_count.toLocaleString()} rows`);
        await record_overture_catalog(client, { file_name: table, region_code: args.bbox, data_date: release, total_count });
      }
      console.log(`[bake/overture] catalog: base tables @ release ${release}`);
    } finally {
      client.release();
    }
  } finally {
    await pool.end?.();
  }
}

await main();
