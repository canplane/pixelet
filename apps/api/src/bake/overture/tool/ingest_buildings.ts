// @pxl/api/bake/overture/tool/ingest_buildings.ts
//
// Pulls Overture's Buildings theme (`building` type only — `building_part`
// skipped, see below) for Korea into overture_buildings.
//
// Confirmed live against a real Gangnam-area sample: only ~14% of Korean
// building features carry `height`/`num_floors`/`class`/`subtype` at all.
// `height`/`num_floors` are kept in the table regardless (a future
// consumer's problem), but baking itself never reads them: ground_baker.ts
// never reads Feature.computed.height (confirmed — nothing in the codebase
// does; it's dead computation), only computed.width for linear features,
// which buildings never are. A structure cell only ever needs to know it's a
// building AND roughly which kind (for cube_types.structure's own small
// enum) — not its real height — so `class`/`subtype` absence just means
// "generic BUILDING", not "unusable row"; `building_part` (sub-footprint
// height slices, e.g. a tower's individual setback tiers) is skipped for
// the same reason: this pipeline has nowhere to put finer height data even
// where it exists.
//
// Same ingest_one()-per-table shape as ingest_divisions.ts, one theme/type
// this time rather than two — see that file's own top comment for the
// shared "why not ogr2ogr/infra-geo" and "why geojsonseq" reasoning.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/overture/tool/ingest_buildings.ts [flags]
//
// Flags:
//   --bbox <minLon,minLat,maxLon,maxLat>  WGS84 box to pull (default: all of South Korea)
//   --out <dir>                           scratch geojsonseq directory (default ./out/overture)

import { Pool, type PoolClient } from "@db/postgres";

import * as env from "@/lib/env/mod.ts";

import { get_overture_release, record_overture_catalog } from "../catalog.ts";

const KR_BBOX_DEFAULT = "124,33,132,39";
const BATCH_SIZE = 200; // polygons are heavier payloads than Places' points

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
DROP TABLE IF EXISTS overture_buildings;
CREATE TABLE overture_buildings (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT,
  class TEXT,
  height DOUBLE PRECISION,
  num_floors INTEGER,
  geom GEOMETRY(Geometry, 3857) NOT NULL
);
CREATE INDEX idx_overture_buildings_geom ON overture_buildings USING GIST (geom);
CREATE INDEX idx_overture_buildings_class ON overture_buildings (class);
`;

type OvertureBuildingFeature = {
  id: string;
  properties: { subtype?: string; class?: string; height?: number; num_floors?: number };
  geometry: unknown;
};

function build_batch_insert(batch: OvertureBuildingFeature[]): { text: string; args: unknown[] } {
  const values: string[] = [];
  const args: unknown[] = [];
  for (const f of batch) {
    const base = args.length;
    args.push(
      f.id,
      f.properties.subtype ?? null,
      f.properties.class ?? null,
      f.properties.height ?? null,
      f.properties.num_floors ?? null,
      JSON.stringify(f.geometry),
    );
    values.push(
      `($${base + 1}, $${base + 2}, $${base + 3}, $${base + 4}, $${base + 5}, ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON($${base + 6}), 4326), 3857))`,
    );
  }
  return {
    text: `INSERT INTO overture_buildings (gers_id, subtype, class, height, num_floors, geom) VALUES ${values.join(",")} ON CONFLICT (gers_id) DO NOTHING`,
    args,
  };
}

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake/overture] bbox=${args.bbox} out=${args.out}`);

  await ensure_overturemaps_cli();
  await Deno.mkdir(args.out, { recursive: true });

  const geojsonseq_path = `${args.out}/buildings.geojsonseq`;
  console.log("[bake/overture] downloading Buildings/building extract from Overture's public S3 release...");
  await run(VENV_OVERTUREMAPS, ["download", `--bbox=${args.bbox}`, "-f", "geojsonseq", "--type=building", "-o", geojsonseq_path]);

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, 4, true);
  try {
    const client: PoolClient = await pool.connect();
    try {
      console.log("[bake/overture] resetting overture_buildings...");
      await client.queryArray(SCHEMA_SQL);

      let batch: OvertureBuildingFeature[] = [];
      let total = 0;

      const flush = async (): Promise<void> => {
        if (batch.length === 0) return;
        const { text, args: qargs } = build_batch_insert(batch);
        await client.queryArray({ text, args: qargs });
        total += batch.length;
        batch = [];
      };

      for await (const line of read_lines(geojsonseq_path)) {
        const f = JSON.parse(line) as OvertureBuildingFeature;
        batch.push(f);
        if (batch.length >= BATCH_SIZE) await flush();
      }
      await flush();

      console.log("[bake/overture] ANALYZE...");
      await client.queryArray("ANALYZE overture_buildings");

      const { rows } = await client.queryObject<{ count: string }>("SELECT count(*) FROM overture_buildings");
      const total_count = Number(rows[0].count);
      console.log(`[bake/overture] done — overture_buildings has ${total_count.toLocaleString()} rows (${total.toLocaleString()} attempted)`);

      const release = await get_overture_release(VENV_OVERTUREMAPS);
      await record_overture_catalog(client, { file_name: "overture_buildings", region_code: args.bbox, data_date: release, total_count });
      console.log(`[bake/overture] catalog: overture_buildings @ release ${release}`);
    } finally {
      client.release();
    }
  } finally {
    await pool.end?.();
  }
}

await main();
