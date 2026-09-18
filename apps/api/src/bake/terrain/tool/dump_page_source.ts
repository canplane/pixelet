// @pxl/api/bake/terrain/tool/dump_page_source.ts
//
// One page's DB source packet and the tile this pipeline bakes from it,
// written side by side on disk.
//
// It exists for the port: crates/pxl/geo-bake carries the same pipeline in
// Rust and has to produce the same bytes, and feeding both the *same* packet
// is what takes the database out of the comparison — the source queries carry
// no ORDER BY, so two runs can see the same rows in a different order and a
// difference in the tile would say nothing about the pipeline.
//
//   deno run -A --env=.env apps/api/src/bake/terrain/tool/dump_page_source.ts <out_dir> <pages_file>
//   cargo run -p pxl-geo-bake --example bake_from_packet -- <out_dir> <page>...
//   cmp <out_dir>/<page>.ts.tile <out_dir>/<page>.rs.tile
//
// `pages_file` is whitespace-separated page indices; out/tiles/land_pages.json
// is where a real set of them comes from.

import { Pool } from "@db/postgres";

import { TerrainRepository } from "@pxl/api/bake/terrain/repository.ts";
import { fetch_terrain_source } from "@pxl/api/bake/terrain/source.ts";
import { bake_terrain } from "@pxl/api/bake/terrain/bake.ts";
import * as env from "@/lib/env/mod.ts";

const [out_dir, pages_file] = Deno.args;
if (!out_dir || !pages_file) {
  console.error("usage: dump_page_source.ts <out_dir> <pages_file>");
  Deno.exit(2);
}

const pages: string[] = (await Deno.readTextFile(pages_file)).trim().split(/\s+/);
const pool = new Pool(env.must("DB_GEO_URL"), 4, true);
const repo = new TerrainRepository(pool);

// fetch_terrain_source reports each page's row counts here; keep the last one
// so the summary line can carry it.
let counts = "";
console.debug = (msg: string) => void (counts = String(msg).replace(/^.*?\] /, ""));

try {
  for (const page of pages) {
    const page_idx = BigInt(page);
    const src = await fetch_terrain_source(repo, page_idx);
    const tile = bake_terrain(page_idx, src);
    await Deno.writeFile(`${out_dir}/${page}.src`, src);
    await Deno.writeFile(`${out_dir}/${page}.ts.tile`, tile);
    console.log(`${page}\t${src.byteLength}\t${tile.byteLength}\t${counts}`);
  }
} finally {
  await pool.end();
}
