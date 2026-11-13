// @pxl/client/abi/cube.spec.ts
//
// GENERATED from spec/spec/asset/cube.json (xpute-gen data.rs)

/** The texture pack, the file the cube kind's palette entry is drawn from: one image
 * of every tile under asset/content/cube/, which the asset ingest composes
 * and ships as the table's file at this path, with which tile each cube
 * wears on which face (pxl-tool bin/asset.rs, ingest.json). In Rust it is
 * the slot the catalog fills with it. */
export const pack = 54 as number;

/** What a cell shows when nothing painted it: a cell with nothing on it is
 * still land, as Carto's background is. */
export const BASE = 1 as Ground;

/** The value space's own: what no palette entry or color is (pxl-spec value.rs). */
export const sentinel = [
  {
    name: "void",
  },
  {
    name: "unknown",
  },
  {
    name: "air",
  },
] as const;

export const Sentinel = {
  /** Nothing is painted: the ground before a bake writes it, and an object cell no leaf holds. */
  void: 0,
  /** Bedrock, where the world holds no more: the floor under the ground and the wall just inside the service area (`pxl-content strata.rs`). A cell past the area or the coverage, which no source here describes, reads as it and is drawn as nothing, and so is a byte no bake wrote. The value is 0xFF so that memory filled with 0xFF reads as unknown. Nothing is placed on it or taken from it. */
  unknown: 255,
  /** Explicit air: the leaf stays and erases the layers under it. */
  air: 256,
} as const;
export type Sentinel = (typeof Sentinel)[keyof typeof Sentinel];

/** The name of the `Sentinel` member numbered `n`, or undefined for a number
 * no member has. */
export function sentinel_name(n: number): keyof typeof Sentinel | undefined {
  for (const k in Sentinel)
    if (Sentinel[k as keyof typeof Sentinel] === n)
      return k as keyof typeof Sentinel;
  return undefined;
}

/** The `Sentinel` member `n` is, or null for a number no member has. */
export function sentinel_of(n: number): Sentinel | null {
  return sentinel_name(n) === undefined ? null : (n as Sentinel);
}

/** The ground: one byte a cell, a palette entry from OpenStreetMap Carto, numbered in
 * the order Carto paints.
 * `transport` marks a way: decor faces along it and no pier stands on it.
 * Whatever the palette entry, a cell's top is the height sampled from the
 * elevation; no entry raises or lowers it. The byte's values are these: 1 to 63
 * are Carto's palette entries, in its paint order; from 64 on are pixelet's own,
 * the underground, of which Carto paints nothing; 0 and 255 are the sentinels. */
export const ground = [
  {
    name: "land",
    label: "Land",
    section: "landcover",
    transport: false,
  },
  {
    name: "bare_ground",
    label: "Bare Ground",
    section: "landcover",
    transport: false,
  },
  {
    name: "sand",
    label: "Sand",
    section: "landcover",
    transport: false,
  },
  {
    name: "beach",
    label: "Beach",
    section: "landcover",
    transport: false,
  },
  {
    name: "heath",
    label: "Heath",
    section: "landcover",
    transport: false,
  },
  {
    name: "scrub",
    label: "Scrub",
    section: "landcover",
    transport: false,
  },
  {
    name: "grass",
    label: "Grass",
    section: "landcover",
    transport: false,
  },
  {
    name: "park",
    label: "Park",
    section: "landcover",
    transport: false,
  },
  {
    name: "leisure",
    label: "Leisure",
    section: "landcover",
    transport: false,
  },
  {
    name: "pitch",
    label: "Pitch",
    section: "landcover",
    transport: false,
  },
  {
    name: "forest",
    label: "Forest",
    section: "landcover",
    transport: false,
  },
  {
    name: "orchard",
    label: "Orchard",
    section: "landcover",
    transport: false,
  },
  {
    name: "allotments",
    label: "Allotments",
    section: "landcover",
    transport: false,
  },
  {
    name: "farmland",
    label: "Farmland",
    section: "landcover",
    transport: false,
  },
  {
    name: "farmyard",
    label: "Farmyard",
    section: "landcover",
    transport: false,
  },
  {
    name: "campsite",
    label: "Campsite",
    section: "landcover",
    transport: false,
  },
  {
    name: "cemetery",
    label: "Cemetery",
    section: "landcover",
    transport: false,
  },
  {
    name: "residential",
    label: "Residential",
    section: "landcover",
    transport: false,
  },
  {
    name: "garages",
    label: "Garages",
    section: "landcover",
    transport: false,
  },
  {
    name: "retail",
    label: "Retail",
    section: "landcover",
    transport: false,
  },
  {
    name: "commercial",
    label: "Commercial",
    section: "landcover",
    transport: false,
  },
  {
    name: "industrial",
    label: "Industrial",
    section: "landcover",
    transport: false,
  },
  {
    name: "power",
    label: "Power",
    section: "landcover",
    transport: false,
  },
  {
    name: "landfill",
    label: "Landfill",
    section: "landcover",
    transport: false,
  },
  {
    name: "quarry",
    label: "Quarry",
    section: "landcover",
    transport: false,
  },
  {
    name: "societal_amenities",
    label: "Societal Amenities",
    section: "landcover",
    transport: false,
  },
  {
    name: "place_of_worship",
    label: "Place of Worship",
    section: "landcover",
    transport: false,
  },
  {
    name: "transportation_area",
    label: "Transportation Area",
    section: "landcover",
    transport: false,
  },
  {
    name: "parking",
    label: "Parking",
    section: "landcover",
    transport: false,
  },
  {
    name: "apron",
    label: "Apron",
    section: "landcover",
    transport: false,
  },
  {
    name: "glacier",
    label: "Glacier",
    section: "water",
    transport: false,
  },
  {
    name: "water",
    label: "Water",
    section: "water",
    transport: false,
  },
  {
    name: "dam",
    label: "Dam",
    section: "water_features",
    transport: false,
  },
  {
    name: "breakwater",
    label: "Breakwater",
    section: "water_features",
    transport: false,
  },
  {
    name: "building",
    label: "Building",
    section: "buildings",
    transport: false,
  },
  {
    name: "building_major",
    label: "Major Building",
    section: "buildings",
    transport: false,
  },
  {
    name: "footway",
    label: "Footway",
    section: "roads",
    transport: true,
  },
  {
    name: "cycleway",
    label: "Cycleway",
    section: "roads",
    transport: true,
  },
  {
    name: "bridleway",
    label: "Bridleway",
    section: "roads",
    transport: true,
  },
  {
    name: "track",
    label: "Track",
    section: "roads",
    transport: true,
  },
  {
    name: "pedestrian",
    label: "Pedestrian Street",
    section: "roads",
    transport: true,
  },
  {
    name: "living_street",
    label: "Living Street",
    section: "roads",
    transport: true,
  },
  {
    name: "residential_road",
    label: "Residential Road",
    section: "roads",
    transport: true,
  },
  {
    name: "road",
    label: "Road",
    section: "roads",
    transport: true,
  },
  {
    name: "secondary",
    label: "Secondary Road",
    section: "roads",
    transport: true,
  },
  {
    name: "primary",
    label: "Primary Road",
    section: "roads",
    transport: true,
  },
  {
    name: "trunk",
    label: "Trunk Road",
    section: "roads",
    transport: true,
  },
  {
    name: "motorway",
    label: "Motorway",
    section: "roads",
    transport: true,
  },
  {
    name: "platform",
    label: "Platform",
    section: "roads",
    transport: true,
  },
  {
    name: "tram",
    label: "Tram",
    section: "roads",
    transport: true,
  },
  {
    name: "light_rail",
    label: "Light Rail",
    section: "roads",
    transport: true,
  },
  {
    name: "subway",
    label: "Subway",
    section: "roads",
    transport: true,
  },
  {
    name: "monorail",
    label: "Monorail",
    section: "roads",
    transport: true,
  },
  {
    name: "rail",
    label: "Railway",
    section: "roads",
    transport: true,
  },
  {
    name: "aeroway",
    label: "Runway",
    section: "aeroways",
    transport: true,
  },
  {
    name: "soil",
    label: "Dirt",
    section: "underground",
    transport: false,
  },
  {
    name: "sandstone",
    label: "Sandstone",
    section: "underground",
    transport: false,
  },
  {
    name: "stone",
    label: "Stone",
    section: "underground",
    transport: false,
  },
] as const;

export const Ground = {
  /** Land no feature covers, and what Carto draws no fill for, takes Carto's background, @land-color. */
  land: 1,
  /** Bare rock, scree and shingle take Carto's @bare_ground. */
  bare_ground: 2,
  /** Sand takes Carto's @sand. */
  sand: 3,
  /** Beach and shoal take Carto's @beach. */
  beach: 4,
  /** Heath and bog take Carto's @heath. */
  heath: 5,
  /** Scrub and mangrove take Carto's @scrub. */
  scrub: 6,
  /** Grass, meadow, garden, village green, marsh and wet meadow take Carto's @grass. */
  grass: 7,
  /** A park takes Carto's @park. */
  park: 8,
  /** A playground, recreation ground, stadium or sports center takes Carto's @leisure, lighten(@park, 5%). */
  leisure: 9,
  /** A pitch or a track takes Carto's @pitch. */
  pitch: 10,
  /** Forest, wood and swamp take Carto's @forest. */
  forest: 11,
  /** An orchard, vineyard or plant nursery takes Carto's @orchard. */
  orchard: 12,
  /** Allotments take Carto's @allotments. */
  allotments: 13,
  /** Farmland and greenhouses take Carto's @farmland. */
  farmland: 14,
  /** A farmyard takes Carto's @farmyard. */
  farmyard: 15,
  /** A campsite or golf course takes Carto's @campsite. */
  campsite: 16,
  /** A cemetery or grave yard takes Carto's @cemetery. */
  cemetery: 17,
  /** Residential land takes Carto's @residential. */
  residential: 18,
  /** Garages take Carto's @garages. */
  garages: 19,
  /** Retail land takes Carto's @retail. */
  retail: 20,
  /** Commercial land takes Carto's @commercial. */
  commercial: 21,
  /** Industrial land, railway land and works take Carto's @industrial. */
  industrial: 22,
  /** A power plant or substation takes Carto's @power, darken(@industrial, 5%). */
  power: 23,
  /** Landfill takes Carto's landuse_landfill, a value with no name. */
  landfill: 24,
  /** A quarry takes Carto's @quarry. */
  quarry: 25,
  /** A school, college, university, hospital or clinic takes Carto's @societal_amenities. */
  societal_amenities: 26,
  /** A religious site takes Carto's @place_of_worship. */
  place_of_worship: 27,
  /** An aerodrome, bus station or ferry terminal takes Carto's @transportation-area. */
  transportation_area: 28,
  /** Parking takes Carto's @parking. */
  parking: 29,
  /** An apron takes Carto's @apron. */
  apron: 30,
  /** A glacier takes Carto's @glacier; Carto draws it with the water areas. */
  glacier: 31,
  /** Sea, lake, river, canal and any other water take Carto's @water-color. */
  water: 32,
  /** A dam takes Carto's @dam. */
  dam: 33,
  /** A breakwater, groyne or weir takes Carto's @breakwater-color or @weir-line. */
  breakwater: 34,
  /** A building takes Carto's @building-fill. */
  building: 35,
  /** A place of worship, a terminal or a station takes Carto's @building-major-fill at z16 and finer, as a cell is z16. */
  building_major: 36,
  /** Footways, steps and paths take Carto's @footway-fill or @steps-fill. */
  footway: 37,
  /** A cycleway takes Carto's @cycleway-fill. */
  cycleway: 38,
  /** A bridleway takes Carto's @bridleway-fill. */
  bridleway: 39,
  /** A track, an agricultural or forest way, takes Carto's @track-fill. */
  track: 40,
  /** A pedestrian street takes Carto's @pedestrian-fill. */
  pedestrian: 41,
  /** A living street takes Carto's @living-street-fill. */
  living_street: 42,
  /** A tertiary, residential, unclassified or service road takes Carto's @tertiary-fill, @residential-fill or @service-fill, told apart by width. */
  residential_road: 43,
  /** A road of no known class takes Carto's @road-fill. */
  road: 44,
  /** A secondary road takes Carto's @secondary-fill. */
  secondary: 45,
  /** A primary road takes Carto's @primary-fill. */
  primary: 46,
  /** A trunk road takes Carto's @trunk-fill. */
  trunk: 47,
  /** A motorway takes Carto's @motorway-fill. */
  motorway: 48,
  /** A platform takes Carto's @platform-fill. */
  platform: 49,
  /** A tram takes Carto's railway_tram, a value with no name. */
  tram: 50,
  /** Light rail, narrow gauge and funicular take Carto's railway_light_rail, a value with no name. */
  light_rail: 51,
  /** A subway takes Carto's railway_subway, a value with no name. */
  subway: 52,
  /** A monorail takes Carto's railway_monorail, a value with no name. */
  monorail: 53,
  /** A railway takes Carto's railway_rail, a value with no name. */
  rail: 54,
  /** A runway, taxiway or helipad takes Carto's @aeroway-fill, @runway-fill, @taxiway-fill or @helipad-fill. */
  aeroway: 55,
  /** What lies under a face: Minecraft Beta's filler, dirt. Carto paints nothing under the ground, and no bake writes it. */
  soil: 64,
  /** What lies under sand's filler: Minecraft Beta's sandstone. */
  sandstone: 65,
  /** What lies under the filler: Minecraft Beta's stone. */
  stone: 66,
} as const;
export type Ground = (typeof Ground)[keyof typeof Ground];

/** The name of the `Ground` member numbered `n`, or undefined for a number
 * no member has. */
export function ground_name(n: number): keyof typeof Ground | undefined {
  for (const k in Ground)
    if (Ground[k as keyof typeof Ground] === n) return k as keyof typeof Ground;
  return undefined;
}

/** The `Ground` member `n` is, or null for a number no member has. */
export function ground_of(n: number): Ground | null {
  return ground_name(n) === undefined ? null : (n as Ground);
}

/** What only an object can be: past a byte, so no ground cell holds it. An
 * object's value is a u16 whose byte range is the ground's palette. */
export const object = [
  {
    name: "white",
    label: "White",
    section: "colors",
  },
  {
    name: "grey",
    label: "Gray",
    section: "colors",
  },
  {
    name: "dark_grey",
    label: "Dark Gray",
    section: "colors",
  },
  {
    name: "black",
    label: "Black",
    section: "colors",
  },
  {
    name: "red",
    label: "Red",
    section: "colors",
  },
  {
    name: "orange",
    label: "Orange",
    section: "colors",
  },
  {
    name: "brown",
    label: "Brown",
    section: "colors",
  },
  {
    name: "yellow",
    label: "Yellow",
    section: "colors",
  },
  {
    name: "green",
    label: "Green",
    section: "colors",
  },
  {
    name: "dark_green",
    label: "Dark Green",
    section: "colors",
  },
  {
    name: "cyan",
    label: "Cyan",
    section: "colors",
  },
  {
    name: "blue",
    label: "Blue",
    section: "colors",
  },
  {
    name: "violet",
    label: "Violet",
    section: "colors",
  },
  {
    name: "magenta",
    label: "Magenta",
    section: "colors",
  },
  {
    name: "pink",
    label: "Pink",
    section: "colors",
  },
] as const;

export const Object = {
  white: 257,
  grey: 258,
  dark_grey: 259,
  black: 260,
  red: 261,
  orange: 262,
  brown: 263,
  yellow: 264,
  green: 265,
  dark_green: 266,
  cyan: 267,
  blue: 268,
  violet: 269,
  magenta: 270,
  pink: 271,
} as const;
export type Object = (typeof Object)[keyof typeof Object];

/** The name of the `Object` member numbered `n`, or undefined for a number
 * no member has. */
export function object_name(n: number): keyof typeof Object | undefined {
  for (const k in Object)
    if (Object[k as keyof typeof Object] === n) return k as keyof typeof Object;
  return undefined;
}

/** The `Object` member `n` is, or null for a number no member has. */
export function object_of(n: number): Object | null {
  return object_name(n) === undefined ? null : (n as Object);
}
