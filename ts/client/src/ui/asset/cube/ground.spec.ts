// @pxl/client/ui/asset/cube/ground.spec.ts
//
// GENERATED from spec/spec/asset/cube.json
// The ground: one byte a cell, a palette entry from OpenStreetMap Carto, numbered in
// the order Carto paints.
// `transport` marks a way: decor faces along it and no pier stands on it.
// Whatever the palette entry, a cell's top is the height sampled from the
// elevation; no entry raises or lowers it. The byte's values are these: 1 to 63
// are Carto's palette entries, in its paint order; from 64 on are pixelet's own,
// the underground, of which Carto paints nothing; 0 and 255 are the sentinels.

// ---- landcover ----

/** Land no feature covers, and what Carto draws no fill for, takes Carto's background, @land-color. */
export const land = 1;

/** Bare rock, scree and shingle take Carto's @bare_ground. */
export const bare_ground = 2;

/** Sand takes Carto's @sand. */
export const sand = 3;

/** Beach and shoal take Carto's @beach. */
export const beach = 4;

/** Heath and bog take Carto's @heath. */
export const heath = 5;

/** Scrub and mangrove take Carto's @scrub. */
export const scrub = 6;

/** Grass, meadow, garden, village green, marsh and wet meadow take Carto's @grass. */
export const grass = 7;

/** A park takes Carto's @park. */
export const park = 8;

/** A playground, recreation ground, stadium or sports center takes Carto's @leisure, lighten(@park, 5%). */
export const leisure = 9;

/** A pitch or a track takes Carto's @pitch. */
export const pitch = 10;

/** Forest, wood and swamp take Carto's @forest. */
export const forest = 11;

/** An orchard, vineyard or plant nursery takes Carto's @orchard. */
export const orchard = 12;

/** Allotments take Carto's @allotments. */
export const allotments = 13;

/** Farmland and greenhouses take Carto's @farmland. */
export const farmland = 14;

/** A farmyard takes Carto's @farmyard. */
export const farmyard = 15;

/** A campsite or golf course takes Carto's @campsite. */
export const campsite = 16;

/** A cemetery or grave yard takes Carto's @cemetery. */
export const cemetery = 17;

/** Residential land takes Carto's @residential. */
export const residential = 18;

/** Garages take Carto's @garages. */
export const garages = 19;

/** Retail land takes Carto's @retail. */
export const retail = 20;

/** Commercial land takes Carto's @commercial. */
export const commercial = 21;

/** Industrial land, railway land and works take Carto's @industrial. */
export const industrial = 22;

/** A power plant or substation takes Carto's @power, darken(@industrial, 5%). */
export const power = 23;

/** Landfill takes Carto's landuse_landfill, a value with no name. */
export const landfill = 24;

/** A quarry takes Carto's @quarry. */
export const quarry = 25;

/** A school, college, university, hospital or clinic takes Carto's @societal_amenities. */
export const societal_amenities = 26;

/** A religious site takes Carto's @place_of_worship. */
export const place_of_worship = 27;

/** An aerodrome, bus station or ferry terminal takes Carto's @transportation-area. */
export const transportation_area = 28;

/** Parking takes Carto's @parking. */
export const parking = 29;

/** An apron takes Carto's @apron. */
export const apron = 30;

// ---- water ----

/** A glacier takes Carto's @glacier; Carto draws it with the water areas. */
export const glacier = 31;

/** Sea, lake, river, canal and any other water take Carto's @water-color. */
export const water = 32;

// ---- water_features ----

/** A dam takes Carto's @dam. */
export const dam = 33;

/** A breakwater, groyne or weir takes Carto's @breakwater-color or @weir-line. */
export const breakwater = 34;

// ---- buildings ----

/** A building takes Carto's @building-fill. */
export const building = 35;

/** A place of worship, a terminal or a station takes Carto's @building-major-fill at z16 and finer, as a cell is z16. */
export const building_major = 36;

// ---- roads ----

/** Footways, steps and paths take Carto's @footway-fill or @steps-fill. */
export const footway = 37;

/** A cycleway takes Carto's @cycleway-fill. */
export const cycleway = 38;

/** A bridleway takes Carto's @bridleway-fill. */
export const bridleway = 39;

/** A track, an agricultural or forest way, takes Carto's @track-fill. */
export const track = 40;

/** A pedestrian street takes Carto's @pedestrian-fill. */
export const pedestrian = 41;

/** A living street takes Carto's @living-street-fill. */
export const living_street = 42;

/** A tertiary, residential, unclassified or service road takes Carto's @tertiary-fill, @residential-fill or @service-fill, told apart by width. */
export const residential_road = 43;

/** A road of no known class takes Carto's @road-fill. */
export const road = 44;

/** A secondary road takes Carto's @secondary-fill. */
export const secondary = 45;

/** A primary road takes Carto's @primary-fill. */
export const primary = 46;

/** A trunk road takes Carto's @trunk-fill. */
export const trunk = 47;

/** A motorway takes Carto's @motorway-fill. */
export const motorway = 48;

/** A platform takes Carto's @platform-fill. */
export const platform = 49;

/** A tram takes Carto's railway_tram, a value with no name. */
export const tram = 50;

/** Light rail, narrow gauge and funicular take Carto's railway_light_rail, a value with no name. */
export const light_rail = 51;

/** A subway takes Carto's railway_subway, a value with no name. */
export const subway = 52;

/** A monorail takes Carto's railway_monorail, a value with no name. */
export const monorail = 53;

/** A railway takes Carto's railway_rail, a value with no name. */
export const rail = 54;

// ---- aeroways ----

/** A runway, taxiway or helipad takes Carto's @aeroway-fill, @runway-fill, @taxiway-fill or @helipad-fill. */
export const aeroway = 55;

// ---- underground ----

/** What lies under a face: Minecraft Beta's filler, dirt. Carto paints nothing under the ground, and no bake writes it. */
export const soil = 64;

/** What lies under sand's filler: Minecraft Beta's sandstone. */
export const sandstone = 65;

/** What lies under the filler: Minecraft Beta's stone. */
export const stone = 66;
