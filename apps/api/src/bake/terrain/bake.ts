// @pxl/api/bake/terrain/bake.ts

import type { f64 } from "@xpute/core/kit/word.ts";

import type { I16Array, U8Array } from "@xpute/core/kit/array.ts";
import { from_tuple } from "@xpute/core/kit/lower.ts";
import { BranchCursor, TreeReader } from "@xpute/core/io/marshal/tree/mod.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";

import { COAST_ROW_KEYS, type CoastRow, encode_terrain, FEAT_ROW_KEYS, type FeatRow, type Ground } from "@pxl/shared/bake/format.ts";
import type { OSMFeature, OSMRawFeature, POI } from "@pxl/shared/bake/model/osm_feature.ts";
import * as osm_adapter from "./model/osm_adapter.ts";
import { normalize, V3_ARENA } from "./pipeline/osm_normalizer.ts";
import { bake_ground } from "./pipeline/ground_baker.ts";
import { extract_pois } from "./pipeline/poi_extractor.ts";

export function bake_terrain(page_idx: IVec, pkt: U8Array): U8Array {
  using _ = V3_ARENA.scope;

  const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
  const page_z_scale: f64 = epsg3857.Z_SCALE(page_bbox[0][1]);

  const root: BranchCursor = new TreeReader(pkt).read_branch();

  /**
   * Elevation: ELE_PADDED_SPAN*ELE_PADDED_SPAN*i16 LE bytes (currently 20*20)
   * OSM Feature/Coastline -> OSMInput[] -> normalize -> bake context
   */

  const [ele, _feats, _coasts] = root.get<[
    ele: I16Array,
    _feats: BranchCursor,
    _coasts: BranchCursor,
  ]>();

  const feats: OSMFeature[] = [];
  for (let i = 0; i < _feats.len; i++) {
    const row: FeatRow = from_tuple(_feats.at_branch(i).get(), FEAT_ROW_KEYS);
    const res: OSMRawFeature[] = osm_adapter.from_feat_row(row);
    for (let j = 0; j < res.length; j++) {
      feats.push(normalize(res[j], page_bbox[0], page_z_scale));
    }
  }
  for (let i = 0; i < _coasts.len; i++) {
    const row: CoastRow = from_tuple(_coasts.at_branch(i).get(), COAST_ROW_KEYS);
    const res: OSMRawFeature[] = osm_adapter.from_coast_row(row);
    for (let j = 0; j < res.length; j++) {
      feats.push(normalize(res[j], page_bbox[0], page_z_scale));
    }
  }

  const ground: Ground = bake_ground(feats);
  const pois: POI[] = extract_pois(feats);

  return encode_terrain(ele, ground, pois);
}
