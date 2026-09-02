// @pxl/shared/spec/cell/terrain.ts
// [Layer: terrain] Tile-loaded 2.5D canvas data

import { CELL_MASKS, common_colors, common_types } from "./common.ts";
import { build_colors_from_json } from "./type.ts";

import colors_json from "@pxl/shared/assets/cell/palette/terrain.json" with { type: "json" };

// ============================================================================
// Type Definitions
// ============================================================================

export const terrain_types = {
  // ---- land ----

  land: {
    ...common_types,

    // ---- basic ----

    /** bare ground */
    BARE: 1,
    /** sea water (MSB-partitioned) */
    SEA: 1 | CELL_MASKS.SEA_FLAG,

    // ---- natural ----

    /** deep green / forest / mountain */
    GREEN_DEEP: 10,
    /** light green / park / grass */
    GREEN_LIGHT: 11,
    /** sand / rock */
    SAND: 12,

    // ---- landuse ----

    /** residential area */
    RESIDENTIAL: 20,
    /** commercial area */
    COMMERCIAL: 21,
    /** industrial area */
    INDUSTRIAL: 22,
    /** sports area */
    SPORTS: 23,
    /** education area */
    EDUCATION: 24,
    /** medical area */
    MEDICAL: 25,
    /** public area */
    PUBLIC: 26,

    // ---- aeroway areas ----

    /** apron */
    APRON: 30,
    /** helipad */
    HELIPAD: 31,
  },

  // ---- water ----

  water: {
    ...common_types,

    /** flowing water */
    RIVER: 1,
    /** still water */
    LAKE: 2,
    /** wetland */
    WETLAND: 3,
  },

  // ---- structure ----

  structure: {
    ...common_types,

    // ---- buildings ----

    /** generic building */
    BUILDING: 1,
    /** station / terminal building */
    BUILDING_STATION: 2,
    /** education building */
    BUILDING_EDUCATION: 3,
    /** medical building */
    BUILDING_MEDICAL: 4,
    /** public building */
    BUILDING_PUBLIC: 5,
  },

  // ---- transport ----

  transport: {
    ...common_types,

    // ---- roads ----

    /** default road */
    ROAD_DEFAULT: 1,
    /** motorway */
    ROAD_MOTORWAY: 2,
    /** primary road */
    ROAD_PRIMARY: 3,
    /** secondary road */
    ROAD_SECONDARY: 4,
    /** pedestrian path */
    PEDESTRIAN: 5,
    /** parking area */
    PARKING: 6,

    // ---- rails ----

    /** main rail */
    RAIL_MAIN: 10,
    /** subway rail */
    RAIL_SUBWAY: 11,

    // ---- aeroway lines ----

    /** runway */
    RUNWAY: 20,
    /** taxiway */
    TAXIWAY: 21,
  },
} as const;

// ============================================================================
// Color Definitions — loaded from JSON (see ../../assets/cell/palette/)
// ============================================================================

export const terrain_colors: Record<string, Record<number, string | null>> = build_colors_from_json(
  terrain_types,
  colors_json,
  common_colors,
);
