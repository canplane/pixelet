// @pxl/api/bake/overture/catalog.ts
//
// Provenance tracking for Overture-sourced tables, shared by every
// bake/overture/tools/ingest_*.ts — mirrors the region_code/data_date/
// file_name/status shape infra/geo's own OSM/elevation catalog tables
// already use (esa_elevation_catalog, osm_coastline_catalog), read by
// bake_util.ts's read_source_catalogs() into every published tile set's
// manifest.json. Without this, a v1/v2 tile version folder name says
// nothing about *when* the underlying data is from — this is what answers
// that, per table, from the DB itself rather than from memory or a commit
// message.
//
// `data_date` holds Overture's own release id (e.g. "2026-08-19.0", from
// `overturemaps releases latest`) rather than the day the ingest happened
// to run — the release id is the actual as-of date of the data snapshot;
// the run date is just when someone got around to pulling it.
//
// Unlike each ingest tool's own feature table (DROP + recreate every run),
// this table is never dropped — it accumulates one row per source table,
// upserted on every re-ingest of that table.

import type { PoolClient } from "@db/postgres";

const SCHEMA_SQL = `
CREATE TABLE IF NOT EXISTS overture_catalog (
  file_name TEXT PRIMARY KEY,   -- which table this row describes, e.g. "overture_buildings"
  region_code TEXT NOT NULL,    -- the --bbox this table was last ingested with
  data_date TEXT NOT NULL,      -- Overture's own release id, e.g. "2026-08-19.0"
  imported_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  status TEXT NOT NULL DEFAULT 'COMPLETED',
  total_count BIGINT NOT NULL DEFAULT 0
);
`;

/** Runs `overturemaps releases latest` and returns the release id, e.g. "2026-08-19.0". */
export async function get_overture_release(venv_overturemaps: string): Promise<string> {
  const p = new Deno.Command(venv_overturemaps, { args: ["releases", "latest"], stdout: "piped", stderr: "inherit" });
  const { success, code, stdout } = await p.output();
  if (!success) throw new Error(`overturemaps releases latest exited ${code}`);
  return new TextDecoder().decode(stdout).trim();
}

export async function record_overture_catalog(
  client: PoolClient,
  entry: { file_name: string; region_code: string; data_date: string; total_count: number },
): Promise<void> {
  await client.queryArray(SCHEMA_SQL);
  await client.queryArray({
    text: `
      INSERT INTO overture_catalog (file_name, region_code, data_date, total_count)
      VALUES ($1, $2, $3, $4)
      ON CONFLICT (file_name) DO UPDATE SET
        region_code = excluded.region_code,
        data_date = excluded.data_date,
        imported_at = now(),
        status = 'COMPLETED',
        total_count = excluded.total_count
    `,
    args: [entry.file_name, entry.region_code, entry.data_date, entry.total_count],
  });
}
