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

import type { u32, u8 } from "@xpute/core/kit/word.ts";
import type { F64Array, I16Array, U16Array, U64Array, U8Array } from "@xpute/core/kit/array.ts";
import { fnv1a64_str } from "@xpute/core/kit/hash.ts";
import { BranchCursor, encoder, TreeReader, TreeView } from "@xpute/core/io/marshal/tree/mod.ts";
import type { POI } from "./model/feature.ts";

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

// ---- POI page ----
//
// One L3 page's places as a record in lanes, read in place:
//
//   [x, y, z, category, rank, id_hash, [categories...], [ids...], [names...], [tags...]]
//
//   x, y, z    f64 per record — page-local centre, real units
//   category   u16 per record — index into `categories`
//   rank       u16 per record — the provider's importance rank
//   id_hash    u64 per record — FNV-1a of the id, what a client keys a place
//              by without holding the string
//   categories str per distinct category, in first-seen order
//   ids, names, tags  str per record; tags as the JSON of the record's tags
//
// The first six are what the client's index and placement read for every
// record every rebuild, and they are typed-array leaves the client views
// without a copy. The rest are read one record at a time — a badge's name
// when it is drawn, a record's tags when it is opened — and never all at
// once. This is the line discussions/20260912-linear-memory-what-is-left.md
// draws: the lanes are the canvas's, the strings are read on demand.

export function encode_poi_page(pois: readonly POI[]): U8Array {
  const n = pois.length;
  const x = new Float64Array(n), y = new Float64Array(n), z = new Float64Array(n);
  const category = new Uint16Array(n), rank = new Uint16Array(n);
  const id_hash = new BigUint64Array(n);
  const categories: string[] = [];
  const category_idx = new Map<string, u32>();
  for (let i = 0; i < n; i++) {
    const poi = pois[i];
    x[i] = poi.center[0];
    y[i] = poi.center[1];
    z[i] = poi.center[2];
    let c = category_idx.get(poi.category);
    if (c === undefined) {
      c = categories.length;
      categories.push(poi.category);
      category_idx.set(poi.category, c);
    }
    category[i] = c;
    rank[i] = Math.min(0xffff, poi.rank);
    id_hash[i] = fnv1a64_str(poi.id);
  }
  return encoder.encode(
    new TreeView().branch((__) =>
      __.f64_array(x).f64_array(y).f64_array(z).u16_array(category).u16_array(rank).u64_array(id_hash)
        .branch((__) => {
          for (const c of categories) __.str(c);
        })
        .branch((__) => {
          for (const poi of pois) __.str(poi.id);
        })
        .branch((__) => {
          for (const poi of pois) __.str(poi.name);
        })
        .branch((__) => {
          for (const poi of pois) __.str(JSON.stringify(poi.tags));
        })
    ),
    { init_cap: 1 << 16 },
  );
}

/**
 * A POI page opened over its packet. The lanes are views into the packet;
 * the strings are cursors read when asked. `pkt` must sit at a word-aligned
 * offset of its buffer, which is what lets the f64 lanes be viewed at all.
 */
export class PoiPage {
  readonly len: u32;
  readonly x: F64Array;
  readonly y: F64Array;
  readonly z: F64Array;
  readonly category: U16Array;
  readonly rank: U16Array;
  readonly id_hash: U64Array;
  /** Decoded once: a page names a few dozen categories at most. */
  readonly categories: readonly string[];
  private readonly ids: BranchCursor;
  private readonly names: BranchCursor;
  private readonly tags: BranchCursor;

  constructor(readonly pkt: U8Array) {
    const root = new TreeReader(pkt).read_branch();
    this.x = root.at(0).get<F64Array>();
    this.y = root.at(1).get<F64Array>();
    this.z = root.at(2).get<F64Array>();
    this.category = root.at(3).get<U16Array>();
    this.rank = root.at(4).get<U16Array>();
    this.id_hash = root.at(5).get<U64Array>();
    this.len = this.x.length;
    this.categories = root.at(6).as_branch().get<string[]>();
    this.ids = root.at(7).as_branch();
    this.names = root.at(8).as_branch();
    this.tags = root.at(9).as_branch();
  }

  category_of(i: u32): string {
    return this.categories[this.category[i]];
  }
  id_of(i: u32): string {
    return this.ids.at(i).get<string>();
  }
  name_of(i: u32): string {
    return this.names.at(i).get<string>();
  }
  tags_of(i: u32): Record<string, string | undefined> {
    return JSON.parse(this.tags.at(i).get<string>());
  }

  /** The record as the object form, for a consumer that wants one — a
   * detail view, a test. One allocation per ask, never per page. */
  record(i: u32): POI {
    return {
      id: this.id_of(i),
      type: "poi",
      category: this.category_of(i),
      name: this.name_of(i),
      center: [this.x[i], this.y[i], this.z[i]],
      tags: this.tags_of(i),
      rank: this.rank[i],
    };
  }
}

/** An empty page: what a client holds for a page the bake found nothing
 * on, so the lanes exist and have length zero. */
export function encode_empty_poi_page(): U8Array {
  return encode_poi_page([]);
}
