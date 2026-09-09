// @pxl/api/bake/overture/model/overture_adapter.ts

/**
 * Overture ingress adapter for WKB-based terrain source rows — the
 * Buildings/Transportation/Base counterpart to coast_adapter.ts. Reuses
 * wkb.ts's decoder as-is. Overture rows carry structured columns straight
 * from Postgres instead of hstore, so there's no tag-parsing step to mirror.
 *
 * Deliberately its own raw-feature shape per theme rather than one shared
 * shape — Overture's subtype/class enums vary per theme, so
 * overture_policy.ts reads these shapes directly instead of through a
 * translation layer.
 */

import type { G3857 } from "@pxl/shared/spec/space/adapter/3857.ts";
import type { BaseRow, BuildingRow, SegmentRow } from "@pxl/shared/bake/format.ts";

import { wkb_to_geometries, type WKBGeometry } from "@pxl/api/bake/wkb.ts";

export const enum OvertureTheme {
  BUILDING = "building",
  SEGMENT = "segment",
  BASE = "base",
}

// -----------------------------------------------------------------------------
// Buildings
// -----------------------------------------------------------------------------

export type OvertureBuildingRawFeature = {
  theme: OvertureTheme.BUILDING;
  gers_id: string;
  subtype: string | null;
  class: string | null;
  height: number | null;
  num_floors: number | null;
  geometry: WKBGeometry;
  center: G3857;
};

export function from_building_row(row: BuildingRow): OvertureBuildingRawFeature[] {
  return wkb_to_geometries(row.geom_wkb).map((g) => ({
    theme: OvertureTheme.BUILDING,
    gers_id: row.gers_id,
    subtype: row.subtype,
    class: row.class,
    height: row.height,
    num_floors: row.num_floors,
    geometry: g,
    center: g.points[0] ?? [0, 0, 0],
  }));
}

// -----------------------------------------------------------------------------
// Transportation (segment only — connector is routing topology, skipped)
// -----------------------------------------------------------------------------

export type OvertureSegmentRawFeature = {
  theme: OvertureTheme.SEGMENT;
  gers_id: string;
  subtype: string;
  class: string | null;
  road: unknown | null;
  geometry: WKBGeometry;
  center: G3857;
};

export function from_segment_row(row: SegmentRow): OvertureSegmentRawFeature[] {
  const road: unknown | null = row.road != null ? JSON.parse(row.road) : null;
  return wkb_to_geometries(row.geom_wkb).map((g) => ({
    theme: OvertureTheme.SEGMENT,
    gers_id: row.gers_id,
    subtype: row.subtype,
    class: row.class,
    road,
    geometry: g,
    center: g.points[0] ?? [0, 0, 0],
  }));
}

// -----------------------------------------------------------------------------
// Base (land_use / water / land / infrastructure — one row shape, four
// tables with different subtype/class vocabularies; `source` tells
// overture_policy.ts which one a given row came from)
// -----------------------------------------------------------------------------

export const enum BaseSource {
  LAND_USE = "land_use",
  WATER = "water",
  LAND = "land",
  INFRASTRUCTURE = "infrastructure",
}

export type OvertureBaseRawFeature = {
  theme: OvertureTheme.BASE;
  gers_id: string;
  source: BaseSource;
  subtype: string | null;
  class: string | null;
  geometry: WKBGeometry;
  center: G3857;
};

export function from_base_row(row: BaseRow): OvertureBaseRawFeature[] {
  const source = row.source as BaseSource; // wire values are exactly BaseSource's own strings — see ingest_base.ts's table set
  return wkb_to_geometries(row.geom_wkb).map((g) => ({
    theme: OvertureTheme.BASE,
    gers_id: row.gers_id,
    source,
    subtype: row.subtype,
    class: row.class,
    geometry: g,
    center: g.points[0] ?? [0, 0, 0],
  }));
}

export type OvertureRawFeature = OvertureBuildingRawFeature | OvertureSegmentRawFeature | OvertureBaseRawFeature;
