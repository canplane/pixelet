// @pxl/api/bake/terrain/pipeline/overture_policy.ts

/**
 * Policy for interpreting Overture Buildings/Transportation/Base rows into
 * categories, metrics, flags, and terrain cell configuration — the
 * counterpart to coast_policy.ts (coastline only).
 *
 * Mapped value-by-value against each theme's own subtype/class vocabulary.
 * Segments/Land_use/Water/Land's class and subtype values below are the
 * ones a real Gangnam-area sample actually produced (this session's own
 * field-population check, 100% populated for all four); Buildings' class
 * enum is not sample-confirmed the same way (only ~14% of Korean buildings
 * carry `class`/`subtype` at all) — mapped against Overture's published
 * buildings schema instead, and it barely matters either way since only
 * the coarse structure category is ever baked (see ingest_buildings.ts's
 * own top comment on `height` being dead weight for rendering).
 */

import type { f64, u32, u8 } from "@xpute/core/kit/word.ts";

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { type CellConfig, GroundChannelKey } from "@pxl/shared/bake/format.ts";
import type { Feature, FeatureExtra } from "@pxl/shared/bake/model/feature.ts";

import { BaseSource, type OvertureRawFeature, OvertureTheme } from "@pxl/api/bake/overture/model/overture_adapter.ts";
import type { NormalizePolicy } from "./normalizer.ts";

// ============ Domain Knowledge Constants ============

/** Overture Transportation `class` -> default width in meters, absent structured `road` lane data. */
const ROAD_WIDTH_BY_CLASS: Record<string, f64> = {
  motorway: 14,
  trunk: 10.5,
  primary: 10.5,
  secondary: 7,
  tertiary: 7,
  residential: 3.5,
  service: 3.5,
  unclassified: 3.5,
  living_street: 3.5,
  path: 1.75,
  footway: 1.75,
  steps: 1.75,
  cycleway: 1.75,
  pedestrian: 1.75,
  standard_gauge: 3.5,
  light_rail: 3.5,
  subway: 3.5,
};

const PEDESTRIAN_CLASSES = new Set(["footway", "path", "steps", "cycleway", "pedestrian", "living_street"]);
const MOTOR_CLASSES = new Set(["motorway", "trunk"]);
const SECONDARY_CLASSES = new Set(["secondary", "tertiary"]);

const FLOWING_WATER_SUBTYPES = new Set(["river", "stream", "canal"]);

const TREE_COVER_LAND_SUBTYPES = new Set(["tree", "forest", "tree_row", "wood"]);

/** Land_use `class` -> land cell, using the values this session's own sample turned up. */
const LAND_USE_CLASS_MAP: Record<string, u8> = {
  residential: cube_types.land.RESIDENTIAL,
  commercial: cube_types.land.COMMERCIAL,
  retail: cube_types.land.COMMERCIAL,
  construction: cube_types.land.INDUSTRIAL,
  park: cube_types.land.GREEN_LIGHT,
  garden: cube_types.land.GREEN_LIGHT,
  grass: cube_types.land.GREEN_LIGHT,
  pedestrian: cube_types.land.GREEN_LIGHT,
  pitch: cube_types.land.SPORTS,
  playground: cube_types.land.SPORTS,
  school: cube_types.land.EDUCATION,
  hospital: cube_types.land.MEDICAL,
};

/** Buildings `class`/`subtype` treated as institutional (against Overture's published schema, not sample-confirmed — see top comment). */
const INSTITUTIONAL_BUILDING_CLASSES = new Set(["education", "medical_care", "civic", "government_service", "religious"]);
const SPORTS_CLASSES = new Set(["pitch", "playground"]);

// ============ Policy Logic ============

function determine_category(feat: OvertureRawFeature): string {
  switch (feat.theme) {
    case OvertureTheme.BUILDING:
      return "building";
    case OvertureTheme.SEGMENT:
      return "road";
    case OvertureTheme.BASE:
      switch (feat.source) {
        case BaseSource.WATER:
          return "water";
        case BaseSource.LAND:
          return TREE_COVER_LAND_SUBTYPES.has(feat.subtype ?? "") ? "forest" : "landuse";
        case BaseSource.INFRASTRUCTURE:
          return feat.class === "parking" ? "parking" : "landuse";
        case BaseSource.LAND_USE:
        default:
          return "landuse";
      }
  }
}

function compute_metrics(feat: OvertureRawFeature, _category: string): Feature["computed"] {
  let height: f64 = 0;
  let width: f64 = 0;

  if (feat.theme === OvertureTheme.BUILDING) {
    // Real height/num_floors when Overture has them; missing both falls
    // back to a single-level default.
    if (feat.height != null) height = feat.height;
    else if (feat.num_floors != null) height = feat.num_floors * 3.5;
    else height = 3.5;
  } else if (feat.theme === OvertureTheme.SEGMENT) {
    width = ROAD_WIDTH_BY_CLASS[feat.class ?? ""] ?? (feat.subtype === "rail" ? 3.5 : 3.5);
  }

  return {
    height: Math.max(0, height),
    width: Math.max(0, width),
    layer: 0, // no bridge/tunnel vertical-stacking signal ingested for Overture yet
  };
}

// Every theme but segments is WKB polygon geometry already (is_polygon_source
// covers closure); should_force_closed has nothing left to force.
function should_force_closed(_feat: OvertureRawFeature): boolean {
  return false;
}

function is_linear(feat: OvertureRawFeature): boolean {
  return feat.theme === OvertureTheme.SEGMENT;
}

function compute_extra(feat: OvertureRawFeature, category: string): FeatureExtra {
  const building_type = feat.theme === OvertureTheme.BUILDING ? (feat.class ?? feat.subtype ?? "") : "";
  const land_use_class = feat.theme === OvertureTheme.BASE && feat.source === BaseSource.LAND_USE ? (feat.class ?? "") : "";

  const is_institutional = INSTITUTIONAL_BUILDING_CLASSES.has(building_type) ||
    land_use_class === "school" || land_use_class === "hospital";

  return {
    rank: calculate_rank(feat, category, is_institutional),
    is_ocean: false, // sea fill is coast_policy.ts's job, never this policy's
    is_parking: category === "parking",
    is_institutional,
    is_sports: SPORTS_CLASSES.has(building_type) || SPORTS_CLASSES.has(land_use_class),
    // Overture's segment `road` JSONB likely carries a tunnel/bridge flag,
    // but its real shape isn't sample-confirmed yet (see this file's top
    // comment on not guessing against unverified schema) — always false
    // until a live sample settles the field path.
    is_tunnel: false,
  };
}

function calculate_rank(feat: OvertureRawFeature, category: string, is_institutional: boolean): u32 {
  let score: u32 = 10;

  if (category === "water") score = 5;
  if (category === "building") score += 10;

  if (feat.theme === OvertureTheme.SEGMENT) {
    const c = feat.class ?? "";
    if (MOTOR_CLASSES.has(c)) score += 80;
    else if (c === "primary" || SECONDARY_CLASSES.has(c)) score += 50;
    else score += 10;
  }

  if (is_institutional) score += 40;

  return score;
}

function determine_cell_cfg(feat: OvertureRawFeature, category: string, extra: FeatureExtra): CellConfig | null {
  if (category === "water" && feat.theme === OvertureTheme.BASE) {
    const cell_type = FLOWING_WATER_SUBTYPES.has(feat.subtype ?? "") ? cube_types.water.RIVER : cube_types.water.LAKE;
    return { cell_type, ch_key: GroundChannelKey.WATER };
  }

  if (category === "building" && feat.theme === OvertureTheme.BUILDING) {
    const building_type = feat.class ?? feat.subtype ?? "";
    let cell_type: u8 = cube_types.structure.BUILDING;
    if (building_type === "transportation") cell_type = cube_types.structure.BUILDING_STATION;
    else if (building_type === "medical_care") cell_type = cube_types.structure.BUILDING_MEDICAL;
    else if (building_type === "education") cell_type = cube_types.structure.BUILDING_EDUCATION;
    else if (extra.is_institutional) cell_type = cube_types.structure.BUILDING_PUBLIC;
    return { cell_type, ch_key: GroundChannelKey.STRUCTURE };
  }

  if (category === "road" && feat.theme === OvertureTheme.SEGMENT) {
    const c = feat.class ?? "";
    let cell_type: u8;
    if (feat.subtype === "rail") {
      cell_type = c === "subway" || c === "light_rail" ? cube_types.transport.RAIL_SUBWAY : cube_types.transport.RAIL_MAIN;
    } else if (PEDESTRIAN_CLASSES.has(c)) {
      cell_type = cube_types.transport.PEDESTRIAN;
    } else if (MOTOR_CLASSES.has(c)) {
      cell_type = cube_types.transport.ROAD_MOTORWAY;
    } else if (c === "primary") {
      cell_type = cube_types.transport.ROAD_PRIMARY;
    } else if (SECONDARY_CLASSES.has(c)) {
      cell_type = cube_types.transport.ROAD_SECONDARY;
    } else {
      cell_type = cube_types.transport.ROAD_DEFAULT;
    }
    return { cell_type, ch_key: GroundChannelKey.TRANSPORT };
  }

  if (category === "parking") {
    return { cell_type: cube_types.transport.PARKING, ch_key: GroundChannelKey.TRANSPORT };
  }

  const land_type = resolve_land(feat, category, extra);
  if (land_type != null) return { cell_type: land_type, ch_key: GroundChannelKey.LAND };

  return null;
}

function resolve_land(feat: OvertureRawFeature, category: string, extra: FeatureExtra): u8 | null {
  if (category === "forest") return cube_types.land.GREEN_DEEP;

  if (feat.theme === OvertureTheme.BASE && feat.source === BaseSource.INFRASTRUCTURE && feat.class === "helipad") {
    return cube_types.land.HELIPAD;
  }

  if (feat.theme === OvertureTheme.BASE && feat.source === BaseSource.LAND_USE) {
    const mapped = LAND_USE_CLASS_MAP[feat.class ?? ""];
    if (mapped != null) return mapped;
    if (extra.is_institutional) return cube_types.land.PUBLIC;
    if (extra.is_sports) return cube_types.land.SPORTS;
    return cube_types.land.BARE;
  }

  return null;
}

export const overture_policy: NormalizePolicy<OvertureRawFeature> = {
  get_id: (raw) => raw.gers_id,
  determine_category,
  compute_metrics,
  compute_extra,
  determine_cell_cfg,
  should_force_closed,
  is_linear,
};
