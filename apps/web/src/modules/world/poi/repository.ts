// @pxl/web/modules/world/poi/repository.ts

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import { ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { POI } from "@pxl/shared/bake/model/osm_feature.ts";

/**
 * POI transport — always the live API, never the pre-baked static tile
 * origin terrain/tile_repository.ts falls back to (2026-09-07 decision: POI
 * data stays off the pre-baked tile format entirely, see
 * discussions/20260907-world-rendering-tiers.md, specifically so shipping a
 * POI change never requires re-baking/re-uploading the whole terrain tile
 * set under a new version prefix). No HAS_STATIC_TILES branch here for that
 * same reason — this always hits `/api/world/poi/:page_idx` on the actual
 * API server.
 */
export async function fetch_poi(page_idx: IVec): Promise<POI[]> {
  const res = await fetch(`/api/world/poi/${page_idx.toString()}`);
  if (!res.ok) throw new Error(`[poi/${ivec.repr(page_idx)}] http ${res.status}`);
  return await res.json() as POI[];
}
