// @pxl/shared/bake/format.ts

/**
 * Terrain format helpers for baked page payloads.
 *
 * The wire field order is defined here.
 *
 * Layer model:
 * - L3 page = terrain transport / storage unit
 * - terrain = static terrain layer carried by that page
 * - ground  = categorical 2D terrain channels
 * - ele     = elevation raster payload for that terrain layer
 *
 * In other words:
 *   L3 page
 *     -> terrain layer
 *          -> ground
 *          -> ele
 *
 * raw wire   = [ele, feats, coasts]
 *   feats  = [[osm_id, tags_hstore, way_wkb, center_wkb], ...]
 *   coasts = [[id, geom_wkb], ...]
 *
 * baked wire = [ele, ground]
 *   ground = [land, water, structure, transport]
 *
 * each ground channel is st.encode(...) => U64Array
 */

import type { I16Array, U8Array } from "@xpute/core/kit/array.ts";
import { BranchCursor, encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";

// import * as json_codec from "@xpute/core/io/json_codec.ts";

import type { POI } from "./model/osm_feature.ts";

// ---- DB ----

export const FEAT_ROW_KEYS = [
  "osm_id",
  "tags_hstore",
  "way_wkb",
  "center_wkb",
] as const;
export type FeatRow = {
  osm_id: string;
  tags_hstore: string | null;
  way_wkb: U8Array; // WKB
  center_wkb: U8Array; // WKB
};

export const COAST_ROW_KEYS = [
  "id",
  "geom_wkb",
  "covers_page",
] as const;
export type CoastRow = {
  id: string;
  geom_wkb: U8Array; // WKB
  covers_page: boolean;
};

// ---- Ground ----

export const enum GroundChannelKey {
  LAND = "land",
  WATER = "water",
  STRUCTURE = "structure",
  TRANSPORT = "transport",
}
export const GROUND_CH_KEYS = [
  GroundChannelKey.LAND,
  GroundChannelKey.WATER,
  GroundChannelKey.STRUCTURE,
  GroundChannelKey.TRANSPORT,
] as const;
export type Ground = Record<GroundChannelKey, U8Array>;

// ---- Page ----

/** Static terrain payload carried by one L3 page. */
export type Terrain = {
  ground: Ground;
  ele: I16Array;
};

/** Decoded page payload with the baked terrain layer attached. */
export type Tile = {
  terrain: Terrain;
};

// ---- DTO ----

// { ele, ground } -> [ele, [land, water, structure, transport]]
export function encode_terrain(ele: I16Array, ground: Ground, _pois: POI[]): U8Array {
  /**
   * Encode baked outputs
   * - ground: sparse-tree encoded ground channels
   * - pois: JSON payloads (compression, if any, should be a storage-layer decision)
   */

  // const pois: U8Array = json_codec.encode(_pois);

  /**
   * Final baked terrain format:
   * - baked = [ele, [land, water, structure, transport]]
   * - each ground channel is an st.encode(...) result
   */
  return encoder.encode(new TreeView().branch((__) =>
    __.i16_array(ele)
      .branch((__) =>
        __.u8_array(ground.land)
          .u8_array(ground.water)
          .u8_array(ground.structure)
          .u8_array(ground.transport)
      )
    // .u8_array(pois)
  ));
}

// [ele, [land, water, structure, transport]] -> { ele, ground }
export function decode_terrain(pkt: U8Array): Terrain {
  const [ele, _ground] = new TreeReader(pkt).read_branch().get<[
    ele: I16Array,
    ground: BranchCursor,
  ]>();

  const [land, water, structure, transport] = _ground.get<[
    land: U8Array,
    water: U8Array,
    structure: U8Array,
    transport: U8Array,
  ]>();

  const ground: Ground = {
    land,
    water,
    structure,
    transport,
  };

  return { ele, ground };
}
