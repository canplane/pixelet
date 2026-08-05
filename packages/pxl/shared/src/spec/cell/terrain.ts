// @pxl/shared/spec/cell/terrain.ts
// [Layer: terrain] Tile-loaded 2.5D canvas data

import { common_colors, common_types } from "./common.ts";
import { CELL_MASKS } from "./utils.ts";

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
// Color Definitions - OSM-like Backup
// ============================================================================

export const terrain_colors_osm = {
  // ---- land ----

  land: {
    ...common_colors,

    [terrain_types.land.BARE]: "#F2EFE9",
    [terrain_types.land.SEA]: "#AACBD6",

    [terrain_types.land.GREEN_DEEP]: "#ADD19E",
    [terrain_types.land.GREEN_LIGHT]: "#C8FACA",
    [terrain_types.land.SAND]: "#F1E6B9",

    [terrain_types.land.RESIDENTIAL]: "#DADADA",
    [terrain_types.land.COMMERCIAL]: "#F2DADE",
    [terrain_types.land.INDUSTRIAL]: "#ECDBE8",
    [terrain_types.land.SPORTS]: "#BCEAC3",
    [terrain_types.land.EDUCATION]: "#FAF7E6",
    [terrain_types.land.MEDICAL]: "#F0F9F5",
    [terrain_types.land.PUBLIC]: "#F0F2F5",

    [terrain_types.land.APRON]: "#E0E0E0",
    [terrain_types.land.HELIPAD]: "#555555",
  },

  // ---- water ----

  water: {
    ...common_colors,

    [terrain_types.water.RIVER]: "#B9DEEA",
    [terrain_types.water.LAKE]: "#A8D3E2",
    [terrain_types.water.WETLAND]: "#D1E3E2",
  },

  // ---- structure ----

  structure: {
    ...common_colors,

    [terrain_types.structure.BUILDING]: "#D9D0C9",
    [terrain_types.structure.BUILDING_STATION]: "#546E7A",
    [terrain_types.structure.BUILDING_MEDICAL]: "#FFFFFF",
    [terrain_types.structure.BUILDING_EDUCATION]: "#F5DEB3",
    [terrain_types.structure.BUILDING_PUBLIC]: "#DFE4EA",
  },

  // ---- transport ----

  transport: {
    ...common_colors,

    [terrain_types.transport.ROAD_DEFAULT]: "#FFFFFF",
    [terrain_types.transport.ROAD_MOTORWAY]: "#E5989B",
    [terrain_types.transport.ROAD_PRIMARY]: "#FAD492",
    [terrain_types.transport.ROAD_SECONDARY]: "#FEFAE0",
    [terrain_types.transport.PEDESTRIAN]: "#DDDDDD",
    [terrain_types.transport.PARKING]: "#C2C2C2",

    [terrain_types.transport.RAIL_MAIN]: "#999999",
    [terrain_types.transport.RAIL_SUBWAY]: "#2A9D8F",

    [terrain_types.transport.RUNWAY]: "#333333",
    [terrain_types.transport.TAXIWAY]: "#555555",
  },
} as const;

// ============================================================================
// Color Definitions - Soft World Theme
// ============================================================================

export const terrain_colors = {
  // ---- land ----

  land: {
    ...common_colors,

    // ---- basic ----

    [terrain_types.land.BARE]: "#F7F1E6", // warm cream ground
    [terrain_types.land.SEA]: "#94D3D8", // soft toy-like sea

    // ---- natural ----

    [terrain_types.land.GREEN_DEEP]: "#9FC58E", // muted forest green
    [terrain_types.land.GREEN_LIGHT]: "#D3EABD", // soft park green
    [terrain_types.land.SAND]: "#F3DEAE", // warm sand

    // ---- landuse ----

    [terrain_types.land.RESIDENTIAL]: "#E1D7CC", // warm light gray
    [terrain_types.land.COMMERCIAL]: "#F5D7CC", // soft peach
    [terrain_types.land.INDUSTRIAL]: "#DDD0D9", // dusty mauve
    [terrain_types.land.SPORTS]: "#BFE0AE", // playful field green
    [terrain_types.land.EDUCATION]: "#F6E6BE", // warm ivory
    [terrain_types.land.MEDICAL]: "#E8F6EE", // clean mint
    [terrain_types.land.PUBLIC]: "#E4E9EE", // cool civic gray

    // ---- aeroway areas ----

    [terrain_types.land.APRON]: "#D9D1C6", // pale concrete
    [terrain_types.land.HELIPAD]: "#6B625B", // muted dark pad
  },

  // ---- water ----

  water: {
    ...common_colors,

    [terrain_types.water.RIVER]: "#BDE8E6", // bright mint river
    [terrain_types.water.LAKE]: "#A5DDD9", // calm soft lake
    [terrain_types.water.WETLAND]: "#CCE4D3", // muted wetland green-blue
  },

  // ---- structure ----

  structure: {
    ...common_colors,

    [terrain_types.structure.BUILDING]: "#D9CBBB", // warm clay-gray
    [terrain_types.structure.BUILDING_STATION]: "#73838C", // blue-gray landmark
    [terrain_types.structure.BUILDING_MEDICAL]: "#FFFDF8", // soft white
    [terrain_types.structure.BUILDING_EDUCATION]: "#EACB98", // warm sand
    [terrain_types.structure.BUILDING_PUBLIC]: "#DDE4E8", // civic cool gray
  },

  // ---- transport ----

  transport: {
    ...common_colors,

    // ---- roads ----

    [terrain_types.transport.ROAD_DEFAULT]: "#F8F1E2", // warm ivory road
    [terrain_types.transport.ROAD_MOTORWAY]: "#E7A1A3", // muted coral
    [terrain_types.transport.ROAD_PRIMARY]: "#F2C78A", // soft amber
    [terrain_types.transport.ROAD_SECONDARY]: "#F7E4AE", // pale butter
    [terrain_types.transport.PEDESTRIAN]: "#E8DED0", // warm pedestrian path
    [terrain_types.transport.PARKING]: "#CEC2B2", // muted parking gray

    // ---- rails ----

    [terrain_types.transport.RAIL_MAIN]: "#8B8176", // warm rail gray
    [terrain_types.transport.RAIL_SUBWAY]: "#64A89F", // softened teal

    // ---- aeroway lines ----

    [terrain_types.transport.RUNWAY]: "#4A443E", // muted charcoal
    [terrain_types.transport.TAXIWAY]: "#70675E", // softened asphalt
  },
} as const;
