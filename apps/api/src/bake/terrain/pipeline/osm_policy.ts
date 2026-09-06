// @pxl/api/bake/terrain/pipeline/osm_policy.ts

/**
 * Central policy file for interpreting raw OSM tags into categories, metrics,
 * flags, and terrain cell configuration.
 *
 * Design goals:
 * - keep lookup tables and interpretation logic together
 * - keep hot-path lookups constant-time
 * - provide stable fallbacks for incomplete source tags
 */

import type { f64, i32, u32, u8 } from "@xpute/core/kit/word.ts";

import { cube_types } from "@pxl/shared/spec/cell/cube.ts";
import { GroundChannelKey } from "@pxl/shared/bake/format.ts";
import type { OSMExtra, OSMFeature, OSMTags } from "@pxl/shared/bake/model/osm_feature.ts";

// ============ Domain Knowledge Constants ============

/**
 * Default lane counts by highway class.
 * Used when the source does not provide an explicit `lanes` tag.
 */
const DEFAULT_LANES_BY_HIGHWAY: Record<string, f64> = {
  motorway: 4,
  trunk: 3,
  primary: 3,
  secondary: 2,
  tertiary: 2,
  residential: 1,
  service: 1,
  unclassified: 1,
  path: 0.5,
  footway: 0.5,
  rail: 1,
};

/**
 * Default aeroway widths in meters.
 * Used when the source does not provide an explicit `width` tag.
 */
const DEFAULT_AEROWAY_WIDTHS: Record<string, f64> = {
  runway: 45,
  taxiway: 23,
  taxilane: 15,
  apron: 0,
};

/**
 * Amenity/building markers treated as institutional facilities.
 */
const INSTITUTIONAL_TYPES = new Set([
  "school",
  "university",
  "college",
  "hospital",
  "public_building",
  "civic_centre",
  "government",
  "train_station",
  "transportation",
]);

const EDU_TYPES = new Set(["school", "university", "college", "kindergarten", "classroom", "dormitory"]);
const MEDICAL_TYPES = new Set(["hospital", "clinic"]);

/** Leisure/sport markers treated as sports facilities. */
const SPORTS_TYPES = new Set(["pitch", "stadium", "track", "golf_course", "sports_centre", "playground"]);

/** Features that should be forced into closed area treatment. */
const FORCE_CLOSE_AMENITIES = new Set(["parking", "school", "university", "hospital"]);
const FORCE_CLOSE_LEISURE = new Set(["pitch", "stadium", "park", "garden"]);

// ============ Policy Logic ============

function determine_road(highway_tag: string): u8 {
  switch (highway_tag) {
    case "motorway":
    case "motorway_link":
    case "trunk":
    case "trunk_link":
      return cube_types.transport.ROAD_MOTORWAY;

    case "primary":
    case "primary_link":
      return cube_types.transport.ROAD_PRIMARY;

    case "secondary":
    case "secondary_link":
    case "tertiary":
    case "tertiary_link":
      return cube_types.transport.ROAD_SECONDARY;

    case "residential":
    case "living_street":
    case "unclassified":
    case "service":
    default:
      return cube_types.transport.ROAD_DEFAULT;
  }
}

/** Maps railway tags to transport cell types. */
function determine_railway(tags: OSMTags): u8 {
  const type = tags.railway;

  if (type === "subway" || type === "light_rail") {
    return cube_types.transport.RAIL_SUBWAY;
  }

  if (type === "rail") {
    return cube_types.transport.RAIL_MAIN;
  }

  return cube_types.transport.RAIL_MAIN;
}

/**
 * Determines the coarse feature category from raw OSM tags.
 *
 * Priority order:
 * 1. Sea
 * 2. Building
 * 3. Inland waters
 * 4. POI
 * 5. Aeroway
 * 6. Road
 * 7. Forest
 * 8. Landuse fallback
 */
export const determine_category = (tags: OSMTags): string => {
  // 🌊 Ocean
  if (
    tags.natural === "coastline" ||
    tags.natural === "ocean" ||
    tags.natural === "sea" ||
    tags.source === "osm_coastline"
  ) {
    return "sea";
  }

  // 🏢 Structure
  if (
    tags.building ||
    tags["building:part"] ||
    tags.man_made === "pier" ||
    (tags.man_made && !["tunnel", "bridge"].includes(tags.man_made))
  ) {
    return "building";
  }

  // 💧 Inland waters
  if (tags.waterway || tags.water || tags.natural === "water" || tags.wetland || ["reservoir", "basin"].includes(tags.landuse || "")) {
    return "water";
  }

  // 📍 POI
  if (tags.amenity || tags.shop || tags.tourism || tags.historic || ["station", "subway_entrance"].includes(tags.railway || "")) {
    return "poi";
  }

  // ✈️ Aeroway
  if (tags.aeroway) {
    if (["apron", "aerodrome", "helipad"].includes(tags.aeroway)) {
      return "landuse";
    }
    return "road";
  }

  // 🛣️ Transport
  if (tags.highway || tags.railway || tags["area:highway"] || tags.junction) {
    return "road";
  }

  // 🌲 Natural, Forest
  if (
    tags.landuse === "forest" ||
    tags.leisure === "nature_reserve" ||
    ["wood", "peak", "mountain", "scrub", "fell", "bare_rock", "cliff", "sand", "beach"].includes(tags.natural || "")
  ) {
    return "forest";
  }

  // 🗺️ Generic (Default)
  return "landuse";
};

/**
 * Computes physical metrics such as height, width, and layer from tags.
 * Height and width are returned in meter-scale source units before local cell conversion.
 */
export const compute_metrics = (tags: OSMTags, category: string): OSMFeature["computed"] => {
  // Height
  let height: f64 = 0;
  if (category === "building") {
    if (tags.height) height = Number.parseFloat(tags.height);
    else {
      const levels: f64 = Number.parseFloat(tags["building:levels"] || "1");
      const min_level: f64 = Number.parseFloat(tags["building:min_level"] || "0");
      const roof_height: f64 = Number.parseFloat(tags["roof:height"] || "0");
      height = (levels - min_level) * 3.5 + roof_height;
    }
  } else if (category === "sea") {
    height = 0;
  }

  // Width
  let width: f64 = 0;
  if (category === "road") {
    if (tags.width) {
      width = Number.parseFloat(tags.width);
    } else if (tags.aeroway) {
      width = DEFAULT_AEROWAY_WIDTHS[tags.aeroway] || 30;
    } else {
      let lanes: f64 = 1;
      if (tags.lanes) {
        lanes = Number.parseInt(tags.lanes, 10);
      } else {
        const type_key = tags.highway || (tags.railway ? "rail" : "unclassified");
        lanes = DEFAULT_LANES_BY_HIGHWAY[type_key] || 1;
      }
      width = lanes * 3.5;
    }
  } else if (category === "water" && tags.waterway) {
    width = tags.width ? Number.parseFloat(tags.width) : 3;
  }

  // Layer
  const base_layer: i32 = category === "sea" ? -5 : 0;
  const layer: i32 = Number.parseInt(tags.layer || base_layer.toString(), 10);

  return {
    height: Math.max(0, isNaN(height) ? 0 : height),
    width: Math.max(0, isNaN(width) ? 0 : width),
    layer: isNaN(layer) ? base_layer : layer,
  };
};

/** Returns true when a feature should be treated as a closed area. */
export const should_force_closed = (tags: OSMTags): boolean => {
  return (
    !!tags.landuse ||
    tags.natural === "ocean" ||
    tags.natural === "sea" ||
    FORCE_CLOSE_AMENITIES.has(tags.amenity || "") ||
    FORCE_CLOSE_LEISURE.has(tags.leisure || "")
  );
};

/**
 * Extracts derived boolean flags and rank used by later bake/render stages.
 */
export const compute_extra = (tags: OSMTags, category: string): OSMExtra => {
  const is_institutional = INSTITUTIONAL_TYPES.has(tags.amenity || "") || tags.railway === "station" ||
    ["train_station", "transportation"].includes(tags.building || "");

  const is_ocean: boolean = tags.source === "osm_coastline" || tags.natural === "ocean" || tags.natural === "sea" || category === "sea";

  return {
    rank: calculate_rank(category, tags, { is_institutional, is_ocean }),

    is_ocean,
    is_parking: category === "parking" || tags.amenity === "parking" || tags.parking === "surface",
    is_institutional,
    is_sports: SPORTS_TYPES.has(tags.leisure || "") || !!tags.sport,
  };
};

/**
 * Computes rendering priority for terrain and POI layering.
 *
 * Policy:
 * - physical layer dominates stylistic hierarchy
 * - more important roads draw after less important roads
 * - named landmarks and institutional features get additional priority
 */
function calculate_rank(category: string, tags: OSMTags, flags: { is_institutional: boolean; is_ocean: boolean }): u32 {
  let score: u32 = 10;

  if (category === "water") score = 5;

  if (category === "poi") score += 50;
  if (category === "building" && tags.name) score += 20;

  // Road hierarchy weights
  if (category === "road") {
    const h = tags.highway || "";

    if (["motorway", "trunk", "motorway_link", "trunk_link"].includes(h)) {
      score += 80;
    } else if (["primary", "primary_link", "secondary", "secondary_link"].includes(h)) {
      score += 50;
    } else if (["tertiary", "tertiary_link"].includes(h)) {
      score += 30;
    } else {
      score += 10;
    }

    const layer: i32 = Number.parseInt(tags.layer || "0", 10);
    score += layer * 100;
  }

  // Flag weights
  if (flags.is_ocean) score += 100;
  if (flags.is_institutional) score += 40;

  // Landmark weights
  if (tags.tourism === "attraction" || tags.heritage) score += 60;
  if (tags.amenity === "hospital" || tags.amenity === "university") score += 30;

  // Named feature boost
  if (tags.name) score += 20;

  return score;
}

// ============ Cell Rendering Policy ============

/**
 * Target terrain channel and cell type resolved for a normalized feature.
 */
export type CellConfig = {
  cell_type: u8;
  ch_key: GroundChannelKey;
};

/**
 * Resolves the target ground channel and cell type for a normalized feature.
 *
 * Returns null when the feature should not be rasterized into terrain channels.
 */
export const determine_cell_cfg = (category: string, tags: OSMTags, extra: OSMExtra): CellConfig | null => {
  // 1. 🏔️ Land: sea and foundation areas
  if (extra.is_ocean || category === "sea") {
    return { cell_type: cube_types.land.SEA, ch_key: GroundChannelKey.LAND };
  }

  // 2. 💧 Water: inland water features
  if (category === "water" || tags.waterway || tags.natural === "water" || tags.wetland || ["reservoir", "basin"].includes(tags.landuse || "")) {
    if (tags.natural === "wetland" || tags.wetland || tags.water === "mud") {
      return { cell_type: cube_types.water.WETLAND, ch_key: GroundChannelKey.WATER };
    }

    const is_flowing = !!tags.waterway || ["river", "canal", "stream", "drain"].includes(tags.water || "");

    const water_value = is_flowing ? cube_types.water.RIVER : cube_types.water.LAKE;
    return { cell_type: water_value, ch_key: GroundChannelKey.WATER };
  }

  // 3. 🏢 Structure: buildings and similar facilities
  if (category === "building" || tags.building) {
    const amenity = tags.amenity || "";
    const building = tags.building || "";

    let cell_type: u8 = cube_types.structure.BUILDING;

    if (
      tags.railway === "station" || tags.public_transport === "station" || amenity === "bus_station" ||
      ["train_station", "transportation", "station"].includes(building)
    ) {
      cell_type = cube_types.structure.BUILDING_STATION;
    } else if (MEDICAL_TYPES.has(amenity) || MEDICAL_TYPES.has(building)) {
      cell_type = cube_types.structure.BUILDING_MEDICAL;
    } else if (EDU_TYPES.has(amenity) || EDU_TYPES.has(building)) {
      cell_type = cube_types.structure.BUILDING_EDUCATION;
    } else if (extra.is_institutional) {
      cell_type = cube_types.structure.BUILDING_PUBLIC;
    }

    return { cell_type, ch_key: GroundChannelKey.STRUCTURE };
  }

  // 4. 🛣️ Transport: roads, rail, runways, parking
  if (category === "road" || tags.highway || tags.railway || category === "parking") {
    let cell_type: u8 = cube_types.transport.ROAD_DEFAULT;

    if (tags.aeroway === "runway") {
      cell_type = cube_types.transport.RUNWAY;
    } else if (tags.aeroway === "taxiway" || tags.aeroway === "taxilane") {
      cell_type = cube_types.transport.TAXIWAY;
    } else if (extra.is_parking) {
      cell_type = cube_types.transport.PARKING;
    } else if (tags.railway) {
      cell_type = determine_railway(tags);
    } else if (["pedestrian", "footway", "living_street", "path"].includes(tags.highway || "")) {
      cell_type = cube_types.transport.PEDESTRIAN;
    } else {
      cell_type = determine_road(tags.highway || "");
    }

    return { cell_type, ch_key: GroundChannelKey.TRANSPORT };
  }

  // 5. 🏔️ Land fallback
  const land_type: u8 | null = resolve_land(category, tags, extra);
  if (land_type) {
    return { cell_type: land_type, ch_key: GroundChannelKey.LAND };
  }

  return null;
};

/** Resolves detailed land-channel cell types. */
function resolve_land(category: string, tags: OSMTags, extra: OSMExtra): u8 | null {
  const category_map: Record<string, u8> = {
    forest: cube_types.land.GREEN_DEEP,
    park: cube_types.land.GREEN_LIGHT,
    sand: cube_types.land.SAND,
  };
  if (category_map[category]) return category_map[category];

  const amenity = tags.amenity || "";
  const landuse = tags.landuse || "";

  if (EDU_TYPES.has(amenity) || landuse === "education") return cube_types.land.EDUCATION;
  if (MEDICAL_TYPES.has(amenity) || landuse === "medical") return cube_types.land.MEDICAL;
  if (extra.is_institutional) return cube_types.land.PUBLIC;
  if (extra.is_sports) return cube_types.land.SPORTS;

  // forest, deep green
  if ((tags.natural && ["wood", "scrub", "heath", "peak", "ridge", "cliff", "mountain", "volcano"].includes(tags.natural)) || tags.landuse === "forest") {
    return cube_types.land.GREEN_DEEP;
  }

  // grass, light green
  if (
    (tags.natural && ["grassland", "fell", "moor"].includes(tags.natural)) ||
    (tags.landuse && ["grass", "village_green", "meadow", "allotments"].includes(tags.landuse)) ||
    (tags.leisure && ["park", "garden", "nature_reserve"].includes(tags.leisure))
  ) {
    return cube_types.land.GREEN_LIGHT;
  }

  // sand, rock
  if ((tags.natural && ["sand", "shingle", "bare_rock", "beach", "scree"].includes(tags.natural)) || tags.landuse === "quarry") {
    return cube_types.land.SAND;
  }

  // human activity
  if (landuse) {
    if (["residential", "cemetery"].includes(landuse)) return cube_types.land.RESIDENTIAL;
    if (["commercial", "retail"].includes(landuse)) return cube_types.land.COMMERCIAL;
    if (["industrial", "construction"].includes(landuse)) return cube_types.land.INDUSTRIAL;
  }

  // sports fallback
  if (tags.leisure && ["pitch", "playground", "golf_course", "track", "stadium"].includes(tags.leisure)) {
    return cube_types.land.SPORTS;
  }

  // airport area
  if (tags.aeroway === "apron") return cube_types.land.APRON;
  if (tags.aeroway === "helipad") return cube_types.land.HELIPAD;
  if (tags.aeroway === "aerodrome") return cube_types.land.BARE;

  // general fallback
  if (tags.landuse || tags.natural || tags.leisure || tags.highway || tags.building) {
    return cube_types.land.BARE;
  }

  return null;
}
