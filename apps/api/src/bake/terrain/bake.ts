// @pxl/api/bake/terrain/bake.ts

import type { f64 } from "@xpute/core/kit/word.ts";

import type { I16Array, U8Array } from "@xpute/core/kit/array.ts";
import { from_tuple } from "@xpute/core/kit/lower.ts";
import { BranchCursor, TreeReader } from "@xpute/core/io/marshal/tree/mod.ts";

import type { IVec } from "@pxl/shared/spec/space/pvf3/mod.ts";
import type { BBox3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import * as epsg3857 from "@pxl/shared/spec/space/adapter/3857.ts";

import {
  BASE_ROW_KEYS,
  type BaseRow,
  BUILDING_ROW_KEYS,
  type BuildingRow,
  COAST_ROW_KEYS,
  type CoastRow,
  encode_terrain,
  type Ground,
  SEGMENT_ROW_KEYS,
  type SegmentRow,
} from "@pxl/shared/bake/format.ts";
import type { CoastRawFeature, Feature } from "@pxl/shared/bake/model/feature.ts";
import { from_coast_row } from "./pipeline/coast_adapter.ts";
import * as overture_adapter from "@pxl/api/bake/overture/model/overture_adapter.ts";
import type { OvertureRawFeature } from "@pxl/api/bake/overture/model/overture_adapter.ts";
import { normalize, V3_ARENA } from "./pipeline/normalizer.ts";
import { coast_policy } from "./pipeline/coast_policy.ts";
import { overture_policy } from "./pipeline/overture_policy.ts";
import { bake_ground } from "./pipeline/ground_baker.ts";

export function bake_terrain(page_idx: IVec, pkt: U8Array): U8Array {
  using _ = V3_ARENA.scope;

  const page_bbox: BBox3857 = epsg3857.get_page_bbox(page_idx);
  const page_z_scale: f64 = epsg3857.Z_SCALE(page_bbox[0][1]);

  const root: BranchCursor = new TreeReader(pkt).read_branch();

  /**
   * Elevation: ELE_PADDED_SPAN*ELE_PADDED_SPAN*i16 LE bytes (currently 68*68)
   * Overture Buildings/Transportation/Base/Coastline -> raw features -> normalize -> bake context
   */

  const [ele, _buildings, _segments, _base, _coasts] = root.get<[
    ele: I16Array,
    _buildings: BranchCursor,
    _segments: BranchCursor,
    _base: BranchCursor,
    _coasts: BranchCursor,
  ]>();

  const feats: Feature[] = [];

  for (let i = 0; i < _buildings.len; i++) {
    const row: BuildingRow = from_tuple(_buildings.at_branch(i).get(), BUILDING_ROW_KEYS);
    const res: OvertureRawFeature[] = overture_adapter.from_building_row(row);
    for (let j = 0; j < res.length; j++) {
      feats.push(normalize(res[j], page_bbox[0], page_z_scale, overture_policy));
    }
  }
  for (let i = 0; i < _segments.len; i++) {
    const row: SegmentRow = from_tuple(_segments.at_branch(i).get(), SEGMENT_ROW_KEYS);
    const res: OvertureRawFeature[] = overture_adapter.from_segment_row(row);
    for (let j = 0; j < res.length; j++) {
      feats.push(normalize(res[j], page_bbox[0], page_z_scale, overture_policy));
    }
  }
  for (let i = 0; i < _base.len; i++) {
    const row: BaseRow = from_tuple(_base.at_branch(i).get(), BASE_ROW_KEYS);
    const res: OvertureRawFeature[] = overture_adapter.from_base_row(row);
    for (let j = 0; j < res.length; j++) {
      feats.push(normalize(res[j], page_bbox[0], page_z_scale, overture_policy));
    }
  }
  for (let i = 0; i < _coasts.len; i++) {
    const row: CoastRow = from_tuple(_coasts.at_branch(i).get(), COAST_ROW_KEYS);
    const res: CoastRawFeature[] = from_coast_row(row);
    for (let j = 0; j < res.length; j++) {
      feats.push(normalize(res[j], page_bbox[0], page_z_scale, coast_policy));
    }
  }

  // whether *any* coastline row touched this page's envelope at all, even
  // if from_coast_row dropped it (covers_page => full land, see that
  // function) — bake_ground needs this to tell "no land data at all (open
  // ocean)" apart from "land data present but not worth rasterizing (full land)".
  const has_coast_row: boolean = _coasts.len > 0;

  const ground: Ground = bake_ground(feats, has_coast_row);

  return encode_terrain(ele, ground);
}
