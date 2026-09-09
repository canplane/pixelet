// @pxl/web/modules/world/terrain/tile_repository.ts

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { decode_terrain, type Terrain } from "@pxl/shared/bake/format.ts";

import { PUBLISHED_TILES, TILE_BASE_URL } from "@/lib/env/config.ts";

/**
 * Terrain transport: turns a page index into decoded terrain, and nothing
 * else. Kept apart from use_tile_fetcher.ts so that "where do tiles come
 * from" and "which tiles should be loaded right now" stay separately
 * readable — the latter is React lifecycle work, this is not.
 */

/** Object key of the single shared open-ocean tile in the static bucket. */
const OCEAN_KEY = "_ocean";

function tile_url(key: string): string {
  return `${TILE_BASE_URL}/${key}`;
}

/**
 * The pre-bake pass (apps/api/src/bake/terrain/tools/bake_tiles.ts) skips
 * pure open-ocean pages entirely — about 65% of the coverage bbox — so the
 * static bucket has no object for them at all. A 404 there isn't a failure,
 * it's the signal for "nothing but water here", and every such page resolves
 * to one shared tile: baking an empty source at different pages was verified
 * to produce identical bytes, so a single decode can be aliased by all of
 * them (terrain is read-only downstream — see render/layer/terrain/adapter.ts).
 *
 * Only reachable when a static tile origin is configured. Against the live
 * API every page, ocean included, bakes to a real 200, so a 404 there is a
 * genuine error and must keep surfacing as one.
 */
let ocean_terrain: Promise<Terrain> | null = null;

function get_ocean_terrain(): Promise<Terrain> {
  if (ocean_terrain === null) {
    ocean_terrain = fetch(tile_url(OCEAN_KEY))
      .then(async (res) => {
        if (!res.ok) throw new Error(`[tile/${OCEAN_KEY}] http ${res.status}`);
        return decode_terrain(new Uint8Array(await res.arrayBuffer()));
      })
      .catch((err) => {
        // don't cache the failure: a transient network error here would
        // otherwise poison every ocean page for the rest of the session.
        ocean_terrain = null;
        throw err;
      });
  }
  return ocean_terrain;
}

/**
 * Fetches and decodes one terrain page. Throws on any failure the caller
 * should see; an unbaked open-ocean page is not one of those. `signal`, when
 * the scheduler passes one, aborts the page request (not the shared ocean
 * fallback, which is one cached fetch aliased by every ocean page).
 *
 * `phases`, when given, is filled with this page's own network-vs-decode
 * split — the numbers that decide whether terrain streaming is actually
 * RTT-bound or memory-bound, which is a different fix in each case (see
 * world/sched/categories.ts). A plain `Record<string, number>` rather than
 * sched's own Phases type on purpose: transport shouldn't need to know the
 * scheduler exists. Left unfilled on the ocean-fallback path, where this
 * page decoded nothing of its own.
 */
export async function fetch_terrain(
  page_idx: IVec,
  signal?: AbortSignal,
  phases?: Record<string, number>,
): Promise<Terrain> {
  const t0 = performance.now();
  const res = await fetch(tile_url(page_idx.toString()), { signal });

  if (res.status === 404 && PUBLISHED_TILES) return get_ocean_terrain();
  if (!res.ok) throw new Error(`[tile/${ivec.repr(page_idx)}] http ${res.status}`);

  const bytes = new Uint8Array(await res.arrayBuffer());
  const t1 = performance.now();
  const terrain = decode_terrain(bytes);

  if (phases) {
    phases.net_ms = t1 - t0;
    phases.decode_ms = performance.now() - t1;
    phases.bytes = bytes.byteLength;
  }
  return terrain;
}
