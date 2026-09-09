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
 * raw wire   = [ele, buildings, segments, base, coasts]
 *   buildings = [[gers_id, subtype, class, height, num_floors, geom_wkb], ...]
 *   segments  = [[gers_id, subtype, class, road, geom_wkb], ...]
 *   base      = [[gers_id, source, subtype, class, geom_wkb], ...]  (source: land_use|water|land|infrastructure)
 *   coasts    = [[id, geom_wkb, covers_page], ...]
 *
 * baked wire = [ele, ground]
 *   ground = [land, water, structure, transport]
 *
 * each ground channel is st.encode(...) => U64Array
 */

import type { u8 } from "@xpute/core/kit/word.ts";
import type { I16Array, U8Array } from "@xpute/core/kit/array.ts";
import { BranchCursor, encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";

// ---- DB ----

export const BUILDING_ROW_KEYS = [
  "gers_id",
  "subtype",
  "class",
  "height",
  "num_floors",
  "geom_wkb",
] as const;
export type BuildingRow = {
  gers_id: string;
  subtype: string | null;
  class: string | null;
  height: number | null;
  num_floors: number | null;
  geom_wkb: U8Array; // WKB
};

export const SEGMENT_ROW_KEYS = [
  "gers_id",
  "subtype",
  "class",
  "road",
  "geom_wkb",
] as const;
export type SegmentRow = {
  gers_id: string;
  subtype: string;
  class: string | null;
  road: string | null; // raw JSON text (::text of the road JSONB column)
  geom_wkb: U8Array; // WKB
};

export const BASE_ROW_KEYS = [
  "gers_id",
  "source",
  "subtype",
  "class",
  "geom_wkb",
] as const;
export type BaseRow = {
  gers_id: string;
  source: string; // "land_use" | "water" | "land" | "infrastructure"
  subtype: string | null;
  class: string | null;
  geom_wkb: U8Array; // WKB
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

/** Target ground channel + cell type resolved by a provider's own policy for one normalized feature. */
export type CellConfig = {
  cell_type: u8;
  ch_key: GroundChannelKey;
};

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
//
// Used to take a third `pois: POI[]` argument and (nominally) encode it
// into the tile too — dead on arrival even then (the actual encode call
// was always commented out) and removed outright once POI moved to its
// own Overture-sourced pipeline (apps/api/src/modules/world/poi/) with no
// relationship to a terrain tile's own bytes at all. `Tile` (below) never
// had a `pois` field to match.
export function encode_terrain(ele: I16Array, ground: Ground): U8Array {
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
