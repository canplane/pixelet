// @pxl/api/bake/bake_util.ts
//
// Small generic helpers shared by every offline bake tool (terrain/tools/
// bake_tiles.ts, poi/tools/bake_poi_tiles.ts) — none of this has any
// terrain- or POI-specific content, extracted here rather than duplicated
// per tool.

import type { Pool } from "@db/postgres";
import type { Bytes } from "@xpute/core/kit/array.ts";

export async function gzip(raw: Bytes): Promise<Bytes> {
  const body = new Response(raw).body!.pipeThrough(new CompressionStream("gzip"));
  return new Uint8Array(await new Response(body).arrayBuffer());
}

export async function exists(path: string): Promise<boolean> {
  try {
    await Deno.stat(path);
    return true;
  } catch {
    return false;
  }
}

export function fmt_bytes(n: number): string {
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} kB`;
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function fmt_dur(ms: number): string {
  const s = Math.round(ms / 1000);
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h}h${m}m` : `${m}m${s % 60}s`;
}

/** The commit that produced a published tile set — the other half of "what is v2":
 * the catalog says which *data* it was baked from, this says which *code*
 * did the baking. Falls back to null rather than throwing when run outside
 * a git checkout (e.g. a stripped CI artifact) — a missing sha in the
 * manifest is a visible gap, not a reason to fail the bake. */
export async function get_git_sha(): Promise<string | null> {
  try {
    const p = new Deno.Command("git", { args: ["rev-parse", "HEAD"], stdout: "piped", stderr: "null" });
    const { success, stdout } = await p.output();
    return success ? new TextDecoder().decode(stdout).trim() : null;
  } catch {
    return null;
  }
}

type CatalogRow = { region_code: string; data_date: string; file_name: string; status: string };

/**
 * Snapshot of what the geo DB was holding when a version was baked. A URL
 * prefix like `v1` is a cache key — deliberately short and stable, since
 * putting a date in it would drag the app build along on every re-bake.
 * That leaves it opaque about *what* v1 actually is, which is what this
 * records instead: the same provenance the ingestion pipelines already
 * track in their catalog tables (see infra/geo/README.md), copied into the
 * manifest so a published tile set can always be traced back to its source
 * data. Shared by every bake tool — they all read the same geo DB.
 */
export async function read_source_catalogs(pool: Pool): Promise<Record<string, CatalogRow[]>> {
  // overture_catalog covers Buildings/Transportation/Base/Places/Divisions/
  // coastline, one row per overture_* table (bake/overture/catalog.ts);
  // elevation is GEDTM30 (esa_elevation_catalog).
  const tables = ["overture_catalog", "esa_elevation_catalog"];
  const out: Record<string, CatalogRow[]> = {};

  for (const table of tables) {
    const client = await pool.connect();
    try {
      const res = await client.queryObject<CatalogRow>(
        `SELECT region_code, data_date::text AS data_date, file_name, status FROM ${table} ORDER BY file_name`,
      );
      out[table] = res.rows;
    } catch (err) {
      // a missing catalog table isn't fatal to the bake — record the gap
      // rather than pretending the provenance is complete.
      console.warn(`[bake/provenance] could not read ${table}:`, (err as Error).message);
      out[table] = [];
    } finally {
      client.release();
    }
  }
  return out;
}
