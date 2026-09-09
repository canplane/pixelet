// @pxl/api/bake/overture/tools/ingest_places.ts
//
// Pulls Overture's Places theme for a bbox (default: all of South Korea)
// from Overture's public S3 release into a dedicated `overture_places`
// table — the one thing overture_provider.ts reads from. Full replace each
// run, not an incremental diff: Overture cuts a new dated release roughly
// monthly rather than streaming edits, so there's nothing to merge against
// (same reasoning bake_poi_tiles.ts's own manifest already documents for
// "this invocation only").
//
// Deliberately its own small pipeline, not folded into infra/geo's
// osm-features/osm-coastline bash+jq+ogr2ogr convention: Overture's
// per-place payload is a rich nested JSON object (categories/names/
// addresses/sources/...), a poor fit for ogr2ogr's flat-field mapping
// (built for OSM's flat tag=value shape and Shapefile's flat attribute
// tables) — and this pipeline is meant to stay structurally independent of
// the terrain pipeline POI used to depend on, not layered on top of it (see
// discussions/20260908-overture-poi-adoption.md).
//
// Attribution: Places is CDLA-Permissive-2.0, which (unlike OSM's ODbL)
// imposes no attribution requirement as a condition of use — still shown
// in-app as a courtesy (apps/web/src/app/settings/sections/
// attribution_section.tsx), matching this project's own norm of crediting
// sources even where not strictly required.
//
// Requires the official `overturemaps` Python CLI (pip), auto-installed on
// first run into a local venv (./.venv, gitignored) next to this file — the
// one new toolchain dependency this brings in, called out explicitly since
// every other bake tool here is pure Deno/TS.
//
// -f geojsonseq (newline-delimited features), not plain geojson: South
// Korea's full Places extract is 700K+ features — a single pretty-printed
// FeatureCollection came to ~900MB and blew Deno's own max-string-length
// loading it with readTextFile+JSON.parse (confirmed live). Streaming line
// by line keeps memory flat regardless of country size.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_places.ts [flags]
//
// Flags:
//   --bbox <minLon,minLat,maxLon,maxLat>  WGS84 box to pull (default: all of South Korea)
//   --out <path>                          scratch geojsonseq path (default ./out/overture/places.geojsonseq)

import { Pool } from "@db/postgres";

import * as env from "@/lib/env/mod.ts";

// Mainland + Jeju + far western islands + Ulleungdo/Dokdo — the same
// overall coverage rationale infra/geo/README.md's ESA Elevation section
// already documents for this project's own Korea bbox.
const KR_BBOX_DEFAULT = "124,33,132,39";
const BATCH_SIZE = 500;

const SCRIPT_DIR = new URL(".", import.meta.url).pathname;
const VENV_DIR = `${SCRIPT_DIR}.venv`;
const VENV_OVERTUREMAPS = `${VENV_DIR}/bin/overturemaps`;

type Args = { bbox: string; out: string };

function parse_args(argv: string[]): Args {
  const args: Args = { bbox: KR_BBOX_DEFAULT, out: "./out/overture/places.geojsonseq" };
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

// Line-by-line, not readTextFile: see this file's own top comment on why —
// a whole-country extract is too large to hold as one JS string.
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

// pg_trgm powers forward search (modules/world/search/providers/
// overture_provider.ts) — a GIN trigram index needs its own plain-text
// column rather than reaching into `properties` per query, so `name` is
// extracted here at ingest time (same names.primary access
// poi/providers/overture_provider.ts's own name_of() does) instead of only
// living inside the JSONB blob.
const SCHEMA_SQL = `
CREATE EXTENSION IF NOT EXISTS postgis;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
DROP TABLE IF EXISTS overture_places;
CREATE TABLE overture_places (
  gers_id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  properties JSONB NOT NULL,
  geom GEOMETRY(Point, 3857) NOT NULL
);
CREATE INDEX idx_overture_places_geom ON overture_places USING GIST (geom);
CREATE INDEX idx_overture_places_properties ON overture_places USING GIN (properties);
CREATE INDEX idx_overture_places_name_trgm ON overture_places USING GIN (name gin_trgm_ops);
`;

type OvertureFeature = {
  id: string;
  properties: Record<string, unknown>;
  geometry: { type: "Point"; coordinates: [number, number] };
};

/** One multi-row INSERT for the whole batch, rather than one round trip per
 * row — the difference between an ingestion that finishes in minutes and
 * one that doesn't finish in a session (confirmed live: row-at-a-time was
 * projected to take longer than this whole build against 700K+ rows). */
function name_of(properties: Record<string, unknown>): string {
  return (properties.names as { primary?: string } | undefined)?.primary ?? "";
}

function build_batch_insert(batch: OvertureFeature[]): { text: string; args: unknown[] } {
  const values: string[] = [];
  const args: unknown[] = [];
  for (const f of batch) {
    const base = args.length;
    const [lon, lat] = f.geometry.coordinates;
    args.push(f.id, name_of(f.properties), JSON.stringify(f.properties), lon, lat);
    values.push(
      `($${base + 1}, $${base + 2}, $${base + 3}::jsonb, ST_Transform(ST_SetSRID(ST_MakePoint($${base + 4}, $${base + 5}), 4326), 3857))`,
    );
  }
  return {
    text: `INSERT INTO overture_places (gers_id, name, properties, geom) VALUES ${values.join(",")} ON CONFLICT (gers_id) DO NOTHING`,
    args,
  };
}

async function main(): Promise<void> {
  const args = parse_args(Deno.args);
  console.log(`[bake/overture] bbox=${args.bbox} out=${args.out}`);

  await ensure_overturemaps_cli();
  await Deno.mkdir(args.out.substring(0, args.out.lastIndexOf("/")), { recursive: true });

  console.log("[bake/overture] downloading Places extract from Overture's public S3 release...");
  await run(VENV_OVERTUREMAPS, ["download", `--bbox=${args.bbox}`, "-f", "geojsonseq", "--type=place", "-o", args.out]);

  const geo_url = env.must("DB_GEO_URL");
  const pool = new Pool(geo_url, 4, true);
  try {
    const client = await pool.connect();
    try {
      console.log("[bake/overture] resetting overture_places...");
      await client.queryArray(SCHEMA_SQL);

      let batch: OvertureFeature[] = [];
      let total = 0;
      const started = performance.now();

      const flush = async (): Promise<void> => {
        if (batch.length === 0) return;
        const { text, args: sql_args } = build_batch_insert(batch);
        await client.queryArray({ text, args: sql_args });
        total += batch.length;
        batch = [];
      };

      for await (const line of read_lines(args.out)) {
        batch.push(JSON.parse(line) as OvertureFeature);
        if (batch.length >= BATCH_SIZE) {
          await flush();
          if (total % (BATCH_SIZE * 40) === 0) {
            const rate = total / ((performance.now() - started) / 1000);
            console.log(`[bake/overture] ${total.toLocaleString()} ingested (${rate.toFixed(0)}/s)`);
          }
        }
      }
      await flush();

      console.log("[bake/overture] ANALYZE...");
      await client.queryArray("ANALYZE overture_places");
      const { rows } = await client.queryObject<{ count: string }>("SELECT count(*) FROM overture_places");
      console.log(`[bake/overture] done — overture_places now has ${Number(rows[0].count).toLocaleString()} rows`);
    } finally {
      client.release();
    }
  } finally {
    await pool.end?.();
  }
}

await main();
