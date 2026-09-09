// @pxl/api/bake/overture/tools/ingest_divisions.ts
//
// Pulls Overture's Divisions theme for Korea into two tables — reverse
// geocoding's own source (apps/api/src/modules/world/geocode/providers/
// overture_provider.ts), same shape as ingest_places.ts's own pipeline for
// POI:
//
//   overture_divisions        (division_area: real boundary polygons)
//   overture_division_points  (division: the entity's own representative
//                              point, present even when nobody has mapped
//                              its polygon yet)
//
// `country` is the only subtype dropped entirely from both. `region`
// (도/특별시) and `county` (시/구) are area-only — Korea's provinces/cities
// always have a mapped polygon, so a point-fallback table entry for them
// would only ever be redundant. `locality`/`neighborhood`/`microhood`/
// `macrohood` — Overture's four sub-city subtypes, all roughly "동" scale
// or finer — are ingested into BOTH tables, because for a lot of Korea a
// polygon simply doesn't exist yet at this level even though the named
// place itself does.
//
// Confirmed live, nationwide: 21,596 `division` points carry subtype
// `locality` vs. only 3,600 `division_area` polygons at that same subtype
// — meaning most of Korea's named localities have NO mapped boundary at
// all. 백현동/수내동 (Bundang/Pangyo, checked by name) are two concrete
// examples: zero rows in `division_area` at ANY subtype, but present in
// `division` as `microhood` points. This is a real upstream OSM boundary-
// coverage gap, not a filter bug here dropping something that exists —
// `overture_provider.ts`'s point-in-polygon query genuinely has nothing to
// find for these, which is why the point table exists as a fallback.
//
// Korea's own administrative depth is also NOT uniform, which is a
// SEPARATE issue from the above coverage gap: a metropolitan city like
// Seoul has no separate city layer under `region` (서울특별시 IS the
// region, 종로구 IS county, 사직동 IS locality — 3 real levels), while a
// province like Gyeonggi-do contains cities that can themselves be split
// into districts (경기도=region, 성남시=county, 분당구=locality — also 3
// levels, but `locality` here lands on a district, not a neighborhood).
// `overture_provider.ts` handles this by always taking whatever's actually
// the finest data available for a given point — polygon match first
// (locality > neighborhood > microhood > macrohood priority), nearest
// point within a radius only when no polygon at any of those four
// subtypes contains it.
//
// Also captures `names.common` (Overture's own per-language name map),
// filtered to this app's supported UI languages (see
// apps/web/src/bootstrap/i18n.ts's own `supportedLngs`) — reverse
// geocoding's own multilingual data, unlike POI's sparse `names.rules[]`.
// Despite the map<LanguageTag,StrippedString> the official schema docs
// describe, the actual GeoJSON encodes `names.common` as an array of
// [lang, name] pairs, not an object — confirmed directly against a raw
// download (e.g. 서울특별시's entry includes `["en","Seoul"]` at some
// arbitrary position among ~100 pairs, not `{en: "Seoul", ...}`).
//
// Same "why not ogr2ogr/infra-geo" and "why geojsonseq" reasoning as
// ingest_places.ts — see that file's own top comment. Unlike Places,
// Divisions genuinely is OSM-derived inside Overture (ODbL, not
// CDLA-Permissive-2.0) — already covered by the existing
// "© OpenStreetMap contributors" notice (attribution_section.tsx), no
// separate wording needed.
//
// Usage (from repo root):
//   deno run -A --env=.env apps/api/src/bake/overture/tools/ingest_divisions.ts [flags]
//
// Flags:
//   --bbox <minLon,minLat,maxLon,maxLat>  WGS84 box to pull (default: all of South Korea)
//   --out <dir>                           scratch geojsonseq directory (default ./out/overture)

import { Pool, type PoolClient } from "@db/postgres";

import * as env from "@/lib/env/mod.ts";

const KR_BBOX_DEFAULT = "124,33,132,39";
const BATCH_SIZE = 200; // polygons/points are heavier payloads than Places'

const AREA_KEPT_SUBTYPES = new Set(["region", "county", "locality", "neighborhood", "microhood", "macrohood"]);
// region/county excluded here on purpose — see the top comment on why a
// point-fallback would be redundant for those two.
const POINT_KEPT_SUBTYPES = new Set(["locality", "neighborhood", "microhood", "macrohood"]);

// apps/web/src/bootstrap/i18n.ts's own supportedLngs — kept in sync by hand
// since api/web are separate Deno workspace members with no shared i18n
// config to import from. Filtering here (not storing the full names.common
// map) keeps baked tile payloads small and avoids re-ingesting just because
// a language got added — worth revisiting if that starts happening often.
const SUPPORTED_LANGS = ["en", "ko"];

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

function schema_sql(table: string, geom_type: string): string {
  return `
CREATE EXTENSION IF NOT EXISTS postgis;
DROP TABLE IF EXISTS ${table};
CREATE TABLE ${table} (
  gers_id TEXT PRIMARY KEY,
  subtype TEXT NOT NULL,
  name TEXT NOT NULL,
  names_common JSONB,
  geom GEOMETRY(${geom_type}, 3857) NOT NULL
);
CREATE INDEX idx_${table}_geom ON ${table} USING GIST (geom);
CREATE INDEX idx_${table}_subtype ON ${table} (subtype);
`;
}

type OvertureDivisionFeature = {
  id: string;
  properties: { subtype?: string; names?: { primary?: string; common?: [string, string][] } };
  geometry: unknown;
};

function pick_supported_common(common: [string, string][] | undefined): Record<string, string> | null {
  if (!common) return null;
  const picked: Record<string, string> = {};
  for (const [lang, name] of common) {
    if (SUPPORTED_LANGS.includes(lang) && name) picked[lang] = name;
  }
  return Object.keys(picked).length > 0 ? picked : null;
}

function build_batch_insert(table: string, batch: OvertureDivisionFeature[]): { text: string; args: unknown[] } {
  const values: string[] = [];
  const args: unknown[] = [];
  for (const f of batch) {
    const base = args.length;
    const names_common = pick_supported_common(f.properties.names?.common);
    args.push(
      f.id,
      f.properties.subtype,
      f.properties.names?.primary ?? "",
      names_common ? JSON.stringify(names_common) : null,
      JSON.stringify(f.geometry),
    );
    values.push(
      `($${base + 1}, $${base + 2}, $${base + 3}, $${base + 4}, ST_Transform(ST_SetSRID(ST_GeomFromGeoJSON($${base + 5}), 4326), 3857))`,
    );
  }
  return {
    text: `INSERT INTO ${table} (gers_id, subtype, name, names_common, geom) VALUES ${values.join(",")} ON CONFLICT (gers_id) DO NOTHING`,
    args,
  };
}

async function ingest_one(
  client: PoolClient,
  overture_type: "division_area" | "division",
  geojsonseq_path: string,
  bbox: string,
  table: string,
  geom_type: string,
  kept_subtypes: Set<string>,
): Promise<void> {
  console.log(`[bake/overture] downloading Divisions/${overture_type} extract from Overture's public S3 release...`);
  await run(VENV_OVERTUREMAPS, ["download", `--bbox=${bbox}`, "-f", "geojsonseq", `--type=${overture_type}`, "-o", geojsonseq_path]);

  console.log(`[bake/overture] resetting ${table}...`);
  await client.queryArray(schema_sql(table, geom_type));

  let batch: OvertureDivisionFeature[] = [];
  let total = 0, skipped = 0;

  const flush = async (): Promise<void> => {
    if (batch.length === 0) return;
    const { text, args } = build_batch_insert(table, batch);
    await client.queryArray({ text, args });
    total += batch.length;
    batch = [];
  };

  for await (const line of read_lines(geojsonseq_path)) {
    const f = JSON.parse(line) as OvertureDivisionFeature;
    if (!f.properties.subtype || !kept_subtypes.has(f.properties.subtype) || !f.properties.names?.primary) {
      skipped++;
      continue;
    }
    batch.push(f);
    if (batch.length >= BATCH_SIZE) await flush();
  }
  await flush();

  console.log(`[bake/overture] ${table}: ${total.toLocaleString()} rows inserted, ${skipped.toLocaleString()} skipped (subtype/unnamed)`);
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
      await ingest_one(
        client,
        "division_area",
        `${args.out}/divisions.geojsonseq`,
        args.bbox,
        "overture_divisions",
        "Geometry",
        AREA_KEPT_SUBTYPES,
      );
      await ingest_one(
        client,
        "division",
        `${args.out}/division_points.geojsonseq`,
        args.bbox,
        "overture_division_points",
        "Point",
        POINT_KEPT_SUBTYPES,
      );

      console.log("[bake/overture] ANALYZE...");
      await client.queryArray("ANALYZE overture_divisions");
      await client.queryArray("ANALYZE overture_division_points");

      const { rows: area_rows } = await client.queryObject<{ count: string }>("SELECT count(*) FROM overture_divisions");
      const { rows: point_rows } = await client.queryObject<{ count: string }>("SELECT count(*) FROM overture_division_points");
      console.log(
        `[bake/overture] done — overture_divisions has ${Number(area_rows[0].count).toLocaleString()} rows, ` +
          `overture_division_points has ${Number(point_rows[0].count).toLocaleString()} rows`,
      );
    } finally {
      client.release();
    }
  } finally {
    await pool.end?.();
  }
}

await main();
