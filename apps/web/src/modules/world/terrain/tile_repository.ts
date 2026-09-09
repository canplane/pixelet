// @pxl/web/modules/world/terrain/tile_repository.ts

import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { decode_terrain, type Terrain } from "@pxl/shared/bake/format.ts";

import { HAS_STATIC_TILES, TILE_BASE_URL } from "@/lib/env/config.ts";

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

/** Fetches and decodes one terrain page. Throws on any failure the caller
 * should see; an unbaked open-ocean page is not one of those. */
export async function fetch_terrain(page_idx: IVec): Promise<Terrain> {
  const res = await fetch(tile_url(page_idx.toString()));

  if (res.status === 404 && HAS_STATIC_TILES) return get_ocean_terrain();
  if (!res.ok) throw new Error(`[tile/${ivec.repr(page_idx)}] http ${res.status}`);

  return decode_terrain(new Uint8Array(await res.arrayBuffer()));
}
