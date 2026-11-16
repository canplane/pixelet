// @pxl/client/abi/cube.spec.ts
//
// GENERATED from spec/spec/asset/cube.json5 (xpute-gen data.rs)

/** The texture pack, the file the cube kind's cubes are drawn from: one image
 * of every tile under asset/content/cube/, which the asset ingest composes
 * and ships as the table's file at this path, with which tile each cube
 * wears on which face (pxl-tool bin/asset.rs, ingest.json5). In Rust it is
 * the slot the catalog fills with it. */
export const pack = 54 as number;

/** What a cell shows when nothing painted it: a cell with nothing on it is
 * still land, as Carto's background is. */
export const BASE = 1 as Ground;

/** The value space's own: what no ground cube or placed color is (pxl-spec value.rs). */
export const sentinel = [
  {
    number: 0 as number,
  },
  {
    number: 255 as number,
  },
  {
    number: 256 as number,
  },
] as const;

export type Sentinel =
  typeof SENTINEL_VOID | typeof SENTINEL_UNKNOWN | typeof SENTINEL_AIR;
/** Nothing is painted: the ground before a bake writes it, and an object cell no leaf holds. */
export const SENTINEL_VOID = 0;
/** Bedrock, where the world holds no more: the floor under the ground and the wall just inside the service area (`pxl-content strata.rs`). A cell past the area or the coverage, which no source here describes, reads as it. It is solid and drawn as the bedrock's layer (pxl-render paint/palette.rs `SENTINELS`). Nothing is placed on it or taken from it. */
export const SENTINEL_UNKNOWN = 255;
/** Explicit air: the leaf stays and erases the layers under it. */
export const SENTINEL_AIR = 256;

/** The member `val` is, or null for a number no member has. */
export function of_Sentinel(val: number): Sentinel | null {
  return val === SENTINEL_VOID ||
    val === SENTINEL_UNKNOWN ||
    val === SENTINEL_AIR
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Sentinel(val: Sentinel): string {
  switch (val) {
    case SENTINEL_VOID:
      return "void";
    case SENTINEL_UNKNOWN:
      return "unknown";
    case SENTINEL_AIR:
      return "air";
  }
}

/** The ground: one byte a cell, its asset id, a ground cube numbered in
 * the order OpenStreetMap Carto paints.
 * `transport` marks a way: decor faces along it and no pier stands on it.
 * Whatever the cube, a cell's top is the height sampled from the
 * elevation; no cube raises or lowers it. The byte's values are these: 1 to 63
 * are cubes in Carto's colors, in its paint order; from 64 on are pixelet's own,
 * the underground, of which Carto paints nothing; 0 and 255 are the sentinels. */
export const ground = [
  {
    number: 1 as number,
    label: "Land",
    section: "landcover",
    transport: false,
  },
  {
    number: 2 as number,
    label: "Bare Ground",
    section: "landcover",
    transport: false,
  },
  {
    number: 3 as number,
    label: "Sand",
    section: "landcover",
    transport: false,
  },
  {
    number: 4 as number,
    label: "Beach",
    section: "landcover",
    transport: false,
  },
  {
    number: 5 as number,
    label: "Heath",
    section: "landcover",
    transport: false,
  },
  {
    number: 6 as number,
    label: "Scrub",
    section: "landcover",
    transport: false,
  },
  {
    number: 7 as number,
    label: "Grass",
    section: "landcover",
    transport: false,
  },
  {
    number: 8 as number,
    label: "Park",
    section: "landcover",
    transport: false,
  },
  {
    number: 9 as number,
    label: "Leisure",
    section: "landcover",
    transport: false,
  },
  {
    number: 10 as number,
    label: "Pitch",
    section: "landcover",
    transport: false,
  },
  {
    number: 11 as number,
    label: "Forest",
    section: "landcover",
    transport: false,
  },
  {
    number: 12 as number,
    label: "Orchard",
    section: "landcover",
    transport: false,
  },
  {
    number: 13 as number,
    label: "Allotments",
    section: "landcover",
    transport: false,
  },
  {
    number: 14 as number,
    label: "Farmland",
    section: "landcover",
    transport: false,
  },
  {
    number: 15 as number,
    label: "Farmyard",
    section: "landcover",
    transport: false,
  },
  {
    number: 16 as number,
    label: "Campsite",
    section: "landcover",
    transport: false,
  },
  {
    number: 17 as number,
    label: "Cemetery",
    section: "landcover",
    transport: false,
  },
  {
    number: 18 as number,
    label: "Residential",
    section: "landcover",
    transport: false,
  },
  {
    number: 19 as number,
    label: "Garages",
    section: "landcover",
    transport: false,
  },
  {
    number: 20 as number,
    label: "Retail",
    section: "landcover",
    transport: false,
  },
  {
    number: 21 as number,
    label: "Commercial",
    section: "landcover",
    transport: false,
  },
  {
    number: 22 as number,
    label: "Industrial",
    section: "landcover",
    transport: false,
  },
  {
    number: 23 as number,
    label: "Power",
    section: "landcover",
    transport: false,
  },
  {
    number: 24 as number,
    label: "Landfill",
    section: "landcover",
    transport: false,
  },
  {
    number: 25 as number,
    label: "Quarry",
    section: "landcover",
    transport: false,
  },
  {
    number: 26 as number,
    label: "Societal Amenities",
    section: "landcover",
    transport: false,
  },
  {
    number: 27 as number,
    label: "Place of Worship",
    section: "landcover",
    transport: false,
  },
  {
    number: 28 as number,
    label: "Transportation Area",
    section: "landcover",
    transport: false,
  },
  {
    number: 29 as number,
    label: "Parking",
    section: "landcover",
    transport: false,
  },
  {
    number: 30 as number,
    label: "Apron",
    section: "landcover",
    transport: false,
  },
  {
    number: 31 as number,
    label: "Glacier",
    section: "water",
    transport: false,
  },
  {
    number: 32 as number,
    label: "Water",
    section: "water",
    transport: false,
  },
  {
    number: 33 as number,
    label: "Dam",
    section: "water_features",
    transport: false,
  },
  {
    number: 34 as number,
    label: "Breakwater",
    section: "water_features",
    transport: false,
  },
  {
    number: 35 as number,
    label: "Building",
    section: "buildings",
    transport: false,
  },
  {
    number: 36 as number,
    label: "Major Building",
    section: "buildings",
    transport: false,
  },
  {
    number: 37 as number,
    label: "Footway",
    section: "roads",
    transport: true,
  },
  {
    number: 38 as number,
    label: "Cycleway",
    section: "roads",
    transport: true,
  },
  {
    number: 39 as number,
    label: "Bridleway",
    section: "roads",
    transport: true,
  },
  {
    number: 40 as number,
    label: "Track",
    section: "roads",
    transport: true,
  },
  {
    number: 41 as number,
    label: "Pedestrian Street",
    section: "roads",
    transport: true,
  },
  {
    number: 42 as number,
    label: "Living Street",
    section: "roads",
    transport: true,
  },
  {
    number: 43 as number,
    label: "Residential Road",
    section: "roads",
    transport: true,
  },
  {
    number: 44 as number,
    label: "Road",
    section: "roads",
    transport: true,
  },
  {
    number: 45 as number,
    label: "Secondary Road",
    section: "roads",
    transport: true,
  },
  {
    number: 46 as number,
    label: "Primary Road",
    section: "roads",
    transport: true,
  },
  {
    number: 47 as number,
    label: "Trunk Road",
    section: "roads",
    transport: true,
  },
  {
    number: 48 as number,
    label: "Motorway",
    section: "roads",
    transport: true,
  },
  {
    number: 49 as number,
    label: "Platform",
    section: "roads",
    transport: true,
  },
  {
    number: 50 as number,
    label: "Tram",
    section: "roads",
    transport: true,
  },
  {
    number: 51 as number,
    label: "Light Rail",
    section: "roads",
    transport: true,
  },
  {
    number: 52 as number,
    label: "Subway",
    section: "roads",
    transport: true,
  },
  {
    number: 53 as number,
    label: "Monorail",
    section: "roads",
    transport: true,
  },
  {
    number: 54 as number,
    label: "Railway",
    section: "roads",
    transport: true,
  },
  {
    number: 55 as number,
    label: "Runway",
    section: "aeroways",
    transport: true,
  },
  {
    number: 64 as number,
    label: "Dirt",
    section: "underground",
    transport: false,
  },
  {
    number: 65 as number,
    label: "Sandstone",
    section: "underground",
    transport: false,
  },
  {
    number: 66 as number,
    label: "Stone",
    section: "underground",
    transport: false,
  },
] as const;

export type Ground =
  | typeof GROUND_LAND
  | typeof GROUND_BARE_GROUND
  | typeof GROUND_SAND
  | typeof GROUND_BEACH
  | typeof GROUND_HEATH
  | typeof GROUND_SCRUB
  | typeof GROUND_GRASS
  | typeof GROUND_PARK
  | typeof GROUND_LEISURE
  | typeof GROUND_PITCH
  | typeof GROUND_FOREST
  | typeof GROUND_ORCHARD
  | typeof GROUND_ALLOTMENTS
  | typeof GROUND_FARMLAND
  | typeof GROUND_FARMYARD
  | typeof GROUND_CAMPSITE
  | typeof GROUND_CEMETERY
  | typeof GROUND_RESIDENTIAL
  | typeof GROUND_GARAGES
  | typeof GROUND_RETAIL
  | typeof GROUND_COMMERCIAL
  | typeof GROUND_INDUSTRIAL
  | typeof GROUND_POWER
  | typeof GROUND_LANDFILL
  | typeof GROUND_QUARRY
  | typeof GROUND_SOCIETAL_AMENITIES
  | typeof GROUND_PLACE_OF_WORSHIP
  | typeof GROUND_TRANSPORTATION_AREA
  | typeof GROUND_PARKING
  | typeof GROUND_APRON
  | typeof GROUND_GLACIER
  | typeof GROUND_WATER
  | typeof GROUND_DAM
  | typeof GROUND_BREAKWATER
  | typeof GROUND_BUILDING
  | typeof GROUND_BUILDING_MAJOR
  | typeof GROUND_FOOTWAY
  | typeof GROUND_CYCLEWAY
  | typeof GROUND_BRIDLEWAY
  | typeof GROUND_TRACK
  | typeof GROUND_PEDESTRIAN
  | typeof GROUND_LIVING_STREET
  | typeof GROUND_RESIDENTIAL_ROAD
  | typeof GROUND_ROAD
  | typeof GROUND_SECONDARY
  | typeof GROUND_PRIMARY
  | typeof GROUND_TRUNK
  | typeof GROUND_MOTORWAY
  | typeof GROUND_PLATFORM
  | typeof GROUND_TRAM
  | typeof GROUND_LIGHT_RAIL
  | typeof GROUND_SUBWAY
  | typeof GROUND_MONORAIL
  | typeof GROUND_RAIL
  | typeof GROUND_AEROWAY
  | typeof GROUND_SOIL
  | typeof GROUND_SANDSTONE
  | typeof GROUND_STONE;
/** Land no feature covers, and what Carto draws no fill for, takes Carto's background, @land-color. */
export const GROUND_LAND = 1;
/** Bare rock, scree and shingle take Carto's @bare_ground. */
export const GROUND_BARE_GROUND = 2;
/** Sand takes Carto's @sand. */
export const GROUND_SAND = 3;
/** Beach and shoal take Carto's @beach. */
export const GROUND_BEACH = 4;
/** Heath and bog take Carto's @heath. */
export const GROUND_HEATH = 5;
/** Scrub and mangrove take Carto's @scrub. */
export const GROUND_SCRUB = 6;
/** Grass, meadow, garden, village green, marsh and wet meadow take Carto's @grass. */
export const GROUND_GRASS = 7;
/** A park takes Carto's @park. */
export const GROUND_PARK = 8;
/** A playground, recreation ground, stadium or sports center takes Carto's @leisure, lighten(@park, 5%). */
export const GROUND_LEISURE = 9;
/** A pitch or a track takes Carto's @pitch. */
export const GROUND_PITCH = 10;
/** Forest, wood and swamp take Carto's @forest. */
export const GROUND_FOREST = 11;
/** An orchard, vineyard or plant nursery takes Carto's @orchard. */
export const GROUND_ORCHARD = 12;
/** Allotments take Carto's @allotments. */
export const GROUND_ALLOTMENTS = 13;
/** Farmland and greenhouses take Carto's @farmland. */
export const GROUND_FARMLAND = 14;
/** A farmyard takes Carto's @farmyard. */
export const GROUND_FARMYARD = 15;
/** A campsite or golf course takes Carto's @campsite. */
export const GROUND_CAMPSITE = 16;
/** A cemetery or grave yard takes Carto's @cemetery. */
export const GROUND_CEMETERY = 17;
/** Residential land takes Carto's @residential. */
export const GROUND_RESIDENTIAL = 18;
/** Garages take Carto's @garages. */
export const GROUND_GARAGES = 19;
/** Retail land takes Carto's @retail. */
export const GROUND_RETAIL = 20;
/** Commercial land takes Carto's @commercial. */
export const GROUND_COMMERCIAL = 21;
/** Industrial land, railway land and works take Carto's @industrial. */
export const GROUND_INDUSTRIAL = 22;
/** A power plant or substation takes Carto's @power, darken(@industrial, 5%). */
export const GROUND_POWER = 23;
/** Landfill takes Carto's landuse_landfill, a value with no name. */
export const GROUND_LANDFILL = 24;
/** A quarry takes Carto's @quarry. */
export const GROUND_QUARRY = 25;
/** A school, college, university, hospital or clinic takes Carto's @societal_amenities. */
export const GROUND_SOCIETAL_AMENITIES = 26;
/** A religious site takes Carto's @place_of_worship. */
export const GROUND_PLACE_OF_WORSHIP = 27;
/** An aerodrome, bus station or ferry terminal takes Carto's @transportation-area. */
export const GROUND_TRANSPORTATION_AREA = 28;
/** Parking takes Carto's @parking. */
export const GROUND_PARKING = 29;
/** An apron takes Carto's @apron. */
export const GROUND_APRON = 30;
/** A glacier takes Carto's @glacier; Carto draws it with the water areas. */
export const GROUND_GLACIER = 31;
/** Sea, lake, river, canal and any other water take Carto's @water-color. */
export const GROUND_WATER = 32;
/** A dam takes Carto's @dam. */
export const GROUND_DAM = 33;
/** A breakwater, groyne or weir takes Carto's @breakwater-color or @weir-line. */
export const GROUND_BREAKWATER = 34;
/** A building takes Carto's @building-fill. */
export const GROUND_BUILDING = 35;
/** A place of worship, a terminal or a station takes Carto's @building-major-fill at z16 and finer, as a cell is z16. */
export const GROUND_BUILDING_MAJOR = 36;
/** Footways, steps and paths take Carto's @footway-fill or @steps-fill. */
export const GROUND_FOOTWAY = 37;
/** A cycleway takes Carto's @cycleway-fill. */
export const GROUND_CYCLEWAY = 38;
/** A bridleway takes Carto's @bridleway-fill. */
export const GROUND_BRIDLEWAY = 39;
/** A track, an agricultural or forest way, takes Carto's @track-fill. */
export const GROUND_TRACK = 40;
/** A pedestrian street takes Carto's @pedestrian-fill. */
export const GROUND_PEDESTRIAN = 41;
/** A living street takes Carto's @living-street-fill. */
export const GROUND_LIVING_STREET = 42;
/** A tertiary, residential, unclassified or service road takes Carto's @tertiary-fill, @residential-fill or @service-fill, told apart by width. */
export const GROUND_RESIDENTIAL_ROAD = 43;
/** A road of no known class takes Carto's @road-fill. */
export const GROUND_ROAD = 44;
/** A secondary road takes Carto's @secondary-fill. */
export const GROUND_SECONDARY = 45;
/** A primary road takes Carto's @primary-fill. */
export const GROUND_PRIMARY = 46;
/** A trunk road takes Carto's @trunk-fill. */
export const GROUND_TRUNK = 47;
/** A motorway takes Carto's @motorway-fill. */
export const GROUND_MOTORWAY = 48;
/** A platform takes Carto's @platform-fill. */
export const GROUND_PLATFORM = 49;
/** A tram takes Carto's railway_tram, a value with no name. */
export const GROUND_TRAM = 50;
/** Light rail, narrow gauge and funicular take Carto's railway_light_rail, a value with no name. */
export const GROUND_LIGHT_RAIL = 51;
/** A subway takes Carto's railway_subway, a value with no name. */
export const GROUND_SUBWAY = 52;
/** A monorail takes Carto's railway_monorail, a value with no name. */
export const GROUND_MONORAIL = 53;
/** A railway takes Carto's railway_rail, a value with no name. */
export const GROUND_RAIL = 54;
/** A runway, taxiway or helipad takes Carto's @aeroway-fill, @runway-fill, @taxiway-fill or @helipad-fill. */
export const GROUND_AEROWAY = 55;
/** What lies under a face: Minecraft Beta's filler, dirt. Carto paints nothing under the ground, and no bake writes it. */
export const GROUND_SOIL = 64;
/** What lies under sand's filler: Minecraft Beta's sandstone. */
export const GROUND_SANDSTONE = 65;
/** What lies under the filler: Minecraft Beta's stone. */
export const GROUND_STONE = 66;

/** The member `val` is, or null for a number no member has. */
export function of_Ground(val: number): Ground | null {
  return val === GROUND_LAND ||
    val === GROUND_BARE_GROUND ||
    val === GROUND_SAND ||
    val === GROUND_BEACH ||
    val === GROUND_HEATH ||
    val === GROUND_SCRUB ||
    val === GROUND_GRASS ||
    val === GROUND_PARK ||
    val === GROUND_LEISURE ||
    val === GROUND_PITCH ||
    val === GROUND_FOREST ||
    val === GROUND_ORCHARD ||
    val === GROUND_ALLOTMENTS ||
    val === GROUND_FARMLAND ||
    val === GROUND_FARMYARD ||
    val === GROUND_CAMPSITE ||
    val === GROUND_CEMETERY ||
    val === GROUND_RESIDENTIAL ||
    val === GROUND_GARAGES ||
    val === GROUND_RETAIL ||
    val === GROUND_COMMERCIAL ||
    val === GROUND_INDUSTRIAL ||
    val === GROUND_POWER ||
    val === GROUND_LANDFILL ||
    val === GROUND_QUARRY ||
    val === GROUND_SOCIETAL_AMENITIES ||
    val === GROUND_PLACE_OF_WORSHIP ||
    val === GROUND_TRANSPORTATION_AREA ||
    val === GROUND_PARKING ||
    val === GROUND_APRON ||
    val === GROUND_GLACIER ||
    val === GROUND_WATER ||
    val === GROUND_DAM ||
    val === GROUND_BREAKWATER ||
    val === GROUND_BUILDING ||
    val === GROUND_BUILDING_MAJOR ||
    val === GROUND_FOOTWAY ||
    val === GROUND_CYCLEWAY ||
    val === GROUND_BRIDLEWAY ||
    val === GROUND_TRACK ||
    val === GROUND_PEDESTRIAN ||
    val === GROUND_LIVING_STREET ||
    val === GROUND_RESIDENTIAL_ROAD ||
    val === GROUND_ROAD ||
    val === GROUND_SECONDARY ||
    val === GROUND_PRIMARY ||
    val === GROUND_TRUNK ||
    val === GROUND_MOTORWAY ||
    val === GROUND_PLATFORM ||
    val === GROUND_TRAM ||
    val === GROUND_LIGHT_RAIL ||
    val === GROUND_SUBWAY ||
    val === GROUND_MONORAIL ||
    val === GROUND_RAIL ||
    val === GROUND_AEROWAY ||
    val === GROUND_SOIL ||
    val === GROUND_SANDSTONE ||
    val === GROUND_STONE
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Ground(val: Ground): string {
  switch (val) {
    case GROUND_LAND:
      return "land";
    case GROUND_BARE_GROUND:
      return "bare_ground";
    case GROUND_SAND:
      return "sand";
    case GROUND_BEACH:
      return "beach";
    case GROUND_HEATH:
      return "heath";
    case GROUND_SCRUB:
      return "scrub";
    case GROUND_GRASS:
      return "grass";
    case GROUND_PARK:
      return "park";
    case GROUND_LEISURE:
      return "leisure";
    case GROUND_PITCH:
      return "pitch";
    case GROUND_FOREST:
      return "forest";
    case GROUND_ORCHARD:
      return "orchard";
    case GROUND_ALLOTMENTS:
      return "allotments";
    case GROUND_FARMLAND:
      return "farmland";
    case GROUND_FARMYARD:
      return "farmyard";
    case GROUND_CAMPSITE:
      return "campsite";
    case GROUND_CEMETERY:
      return "cemetery";
    case GROUND_RESIDENTIAL:
      return "residential";
    case GROUND_GARAGES:
      return "garages";
    case GROUND_RETAIL:
      return "retail";
    case GROUND_COMMERCIAL:
      return "commercial";
    case GROUND_INDUSTRIAL:
      return "industrial";
    case GROUND_POWER:
      return "power";
    case GROUND_LANDFILL:
      return "landfill";
    case GROUND_QUARRY:
      return "quarry";
    case GROUND_SOCIETAL_AMENITIES:
      return "societal_amenities";
    case GROUND_PLACE_OF_WORSHIP:
      return "place_of_worship";
    case GROUND_TRANSPORTATION_AREA:
      return "transportation_area";
    case GROUND_PARKING:
      return "parking";
    case GROUND_APRON:
      return "apron";
    case GROUND_GLACIER:
      return "glacier";
    case GROUND_WATER:
      return "water";
    case GROUND_DAM:
      return "dam";
    case GROUND_BREAKWATER:
      return "breakwater";
    case GROUND_BUILDING:
      return "building";
    case GROUND_BUILDING_MAJOR:
      return "building_major";
    case GROUND_FOOTWAY:
      return "footway";
    case GROUND_CYCLEWAY:
      return "cycleway";
    case GROUND_BRIDLEWAY:
      return "bridleway";
    case GROUND_TRACK:
      return "track";
    case GROUND_PEDESTRIAN:
      return "pedestrian";
    case GROUND_LIVING_STREET:
      return "living_street";
    case GROUND_RESIDENTIAL_ROAD:
      return "residential_road";
    case GROUND_ROAD:
      return "road";
    case GROUND_SECONDARY:
      return "secondary";
    case GROUND_PRIMARY:
      return "primary";
    case GROUND_TRUNK:
      return "trunk";
    case GROUND_MOTORWAY:
      return "motorway";
    case GROUND_PLATFORM:
      return "platform";
    case GROUND_TRAM:
      return "tram";
    case GROUND_LIGHT_RAIL:
      return "light_rail";
    case GROUND_SUBWAY:
      return "subway";
    case GROUND_MONORAIL:
      return "monorail";
    case GROUND_RAIL:
      return "rail";
    case GROUND_AEROWAY:
      return "aeroway";
    case GROUND_SOIL:
      return "soil";
    case GROUND_SANDSTONE:
      return "sandstone";
    case GROUND_STONE:
      return "stone";
  }
}

/** What only an object can be: past a byte, so no ground cell holds it. An
 * object's value is a u16 whose byte range is the ground's. */
export const object = [
  {
    number: 257 as number,
    label: "White",
    section: "colors",
  },
  {
    number: 258 as number,
    label: "Gray",
    section: "colors",
  },
  {
    number: 259 as number,
    label: "Dark Gray",
    section: "colors",
  },
  {
    number: 260 as number,
    label: "Black",
    section: "colors",
  },
  {
    number: 261 as number,
    label: "Red",
    section: "colors",
  },
  {
    number: 262 as number,
    label: "Orange",
    section: "colors",
  },
  {
    number: 263 as number,
    label: "Brown",
    section: "colors",
  },
  {
    number: 264 as number,
    label: "Yellow",
    section: "colors",
  },
  {
    number: 265 as number,
    label: "Green",
    section: "colors",
  },
  {
    number: 266 as number,
    label: "Dark Green",
    section: "colors",
  },
  {
    number: 267 as number,
    label: "Cyan",
    section: "colors",
  },
  {
    number: 268 as number,
    label: "Blue",
    section: "colors",
  },
  {
    number: 269 as number,
    label: "Violet",
    section: "colors",
  },
  {
    number: 270 as number,
    label: "Magenta",
    section: "colors",
  },
  {
    number: 271 as number,
    label: "Pink",
    section: "colors",
  },
] as const;

export type Object =
  | typeof OBJECT_WHITE
  | typeof OBJECT_GREY
  | typeof OBJECT_DARK_GREY
  | typeof OBJECT_BLACK
  | typeof OBJECT_RED
  | typeof OBJECT_ORANGE
  | typeof OBJECT_BROWN
  | typeof OBJECT_YELLOW
  | typeof OBJECT_GREEN
  | typeof OBJECT_DARK_GREEN
  | typeof OBJECT_CYAN
  | typeof OBJECT_BLUE
  | typeof OBJECT_VIOLET
  | typeof OBJECT_MAGENTA
  | typeof OBJECT_PINK;
export const OBJECT_WHITE = 257;
export const OBJECT_GREY = 258;
export const OBJECT_DARK_GREY = 259;
export const OBJECT_BLACK = 260;
export const OBJECT_RED = 261;
export const OBJECT_ORANGE = 262;
export const OBJECT_BROWN = 263;
export const OBJECT_YELLOW = 264;
export const OBJECT_GREEN = 265;
export const OBJECT_DARK_GREEN = 266;
export const OBJECT_CYAN = 267;
export const OBJECT_BLUE = 268;
export const OBJECT_VIOLET = 269;
export const OBJECT_MAGENTA = 270;
export const OBJECT_PINK = 271;

/** The member `val` is, or null for a number no member has. */
export function of_Object(val: number): Object | null {
  return val === OBJECT_WHITE ||
    val === OBJECT_GREY ||
    val === OBJECT_DARK_GREY ||
    val === OBJECT_BLACK ||
    val === OBJECT_RED ||
    val === OBJECT_ORANGE ||
    val === OBJECT_BROWN ||
    val === OBJECT_YELLOW ||
    val === OBJECT_GREEN ||
    val === OBJECT_DARK_GREEN ||
    val === OBJECT_CYAN ||
    val === OBJECT_BLUE ||
    val === OBJECT_VIOLET ||
    val === OBJECT_MAGENTA ||
    val === OBJECT_PINK
    ? val
    : null;
}

/** The member's name, as its source spells it. */
export function name_Object(val: Object): string {
  switch (val) {
    case OBJECT_WHITE:
      return "white";
    case OBJECT_GREY:
      return "grey";
    case OBJECT_DARK_GREY:
      return "dark_grey";
    case OBJECT_BLACK:
      return "black";
    case OBJECT_RED:
      return "red";
    case OBJECT_ORANGE:
      return "orange";
    case OBJECT_BROWN:
      return "brown";
    case OBJECT_YELLOW:
      return "yellow";
    case OBJECT_GREEN:
      return "green";
    case OBJECT_DARK_GREEN:
      return "dark_green";
    case OBJECT_CYAN:
      return "cyan";
    case OBJECT_BLUE:
      return "blue";
    case OBJECT_VIOLET:
      return "violet";
    case OBJECT_MAGENTA:
      return "magenta";
    case OBJECT_PINK:
      return "pink";
  }
}
