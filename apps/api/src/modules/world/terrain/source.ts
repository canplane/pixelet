// @pxl/api/modules/world/terrain/source.ts

import type { f64, i16, u32 } from "@xpute/core/kit/word.ts";
import type { Bytes, I16Array } from "@xpute/core/kit/array.ts";
import { to_tuple } from "@xpute/core/kit/lower.ts";
import { encoder, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";

import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";
import { type IVec, ivec } from "@pxl/shared/spec/space/pvf3/mod.ts";

import { COAST_ROW_KEYS, type CoastRow, FEAT_ROW_KEYS, type FeatRow } from "@pxl/shared/bake/format.ts";
import { ELE_PADDED_SPAN } from "@pxl/shared/bake/constants.ts";
import { bake_terrain } from "@pxl/api/bake/terrain/bake.ts";
import type { TerrainRepository } from "./repository.ts";

const ELE_PADDED_SPAN2: u32 = ELE_PADDED_SPAN * ELE_PADDED_SPAN;

/**
 * Counters for degradations that are survivable per request but matter in bulk.
 *
 * `invalid_ele_shape` in particular: a page whose elevation query comes back the
 * wrong length silently gets a zero-filled grid and bakes perfectly flat. One
 * such page is a warning line nobody reads; several hundred is a visible
 * property of the dataset, and an offline bake over tens of thousands of pages
 * has no other way to notice. The offline tool reports this in its summary —
 * see bake/terrain/tools/bake_tiles.ts.
 */
export const source_stats = { invalid_ele_shape: 0 };

/**
 * Assembles one page's raw DB source (elevation raster + OSM feature rows +
 * coastline rows) into the packed tree the bake step consumes.
 *
 * Lives here rather than inside service.ts's closure because the offline
 * pre-bake tool (bake/terrain/tools/bake_tiles.ts) has to produce *byte
 * identical* tiles to what the live endpoint serves — the CDN copy and the
 * live copy are supposed to be interchangeable, so they can't be allowed to
 * drift by way of two independent assembly paths.
 */
export async function fetch_terrain_source(repo: TerrainRepository, page_idx: IVec): Promise<Bytes> {
  const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
  const page_z_scale: f64 = epsg3857.Z_SCALE(page_bbox[0][1]);

  // DB fetch
  let _ele: i16[] | null;
  let feat_rows: FeatRow[];
  let coast_rows: CoastRow[];
  try {
    [_ele, feat_rows, coast_rows] = await Promise.all([
      repo.fetch_ele(page_bbox, page_z_scale),
      repo.fetch_feat_rows(page_bbox),
      repo.fetch_coast_rows(page_bbox),
    ]);
  } catch (err) {
    console.error(`[terrain/source/${ivec.repr(page_idx)}] db error:`, err);
    throw err;
  }

  let ele: I16Array;
  if (_ele && _ele.length === ELE_PADDED_SPAN2) {
    ele = new Int16Array(_ele);
  } else {
    source_stats.invalid_ele_shape++;
    console.warn(`[terrain/source/${ivec.repr(page_idx)}] invalid ele shape`, _ele?.length ?? null);
    ele = new Int16Array(ELE_PADDED_SPAN2); // fallback
  }

  console.debug(
    `[terrain/source/${ivec.repr(page_idx)}] ele=${ele.length}, feat_rows=${feat_rows.length}, coast_rows=${coast_rows.length}`,
  );

  const feats_view = new TreeView().branch((__) => {
    for (const row of feat_rows) {
      __.put(to_tuple(row, FEAT_ROW_KEYS));
    }
  });
  const coasts_view = new TreeView().branch((__) => {
    for (const row of coast_rows) {
      __.put(to_tuple(row, COAST_ROW_KEYS));
    }
  });

  return encoder.encode(new TreeView().branch((__) =>
    __.i16_array(ele)
      .subtree(feats_view)
      .subtree(coasts_view)
  ));
}

/**
 * Raw OSM/elevation source, baked into client-ready ground channels.
 * This step used to run inside the (now-dropped) WASM worker.
 */
export async function fetch_tile(repo: TerrainRepository, page_idx: IVec): Promise<Bytes> {
  const raw: Bytes = await fetch_terrain_source(repo, page_idx);
  return bake_terrain(page_idx, raw);
}
