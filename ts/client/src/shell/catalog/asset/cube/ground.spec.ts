// @pxl/client/shell/catalog/asset/cube/ground.spec.ts
//
// GENERATED from spec/spec/asset/cube.json
// The ground: one byte a cell, a look from OpenStreetMap Carto, numbered in
// the order Carto paints.
// `transport` marks a way: decor faces along it and no pier stands on it;
// every look stands at its sampled height. The byte's values: 1 to 63
// Carto's looks, in its paint order; from 64 pixelet's own, the
// underground, which Carto paints nothing of; 0 and 255 the sentinels.

// ---- landcover ----

/** land no feature covers, and what Carto draws no fill for: Carto's background, @land-color */
export const land = 1;

/** bare rock, scree, shingle: Carto @bare_ground */
export const bare_ground = 2;

/** sand: Carto @sand */
export const sand = 3;

/** beach and shoal: Carto @beach */
export const beach = 4;

/** heath and bog: Carto @heath */
export const heath = 5;

/** scrub and mangrove: Carto @scrub */
export const scrub = 6;

/** grass, meadow, garden, village green, marsh, wet meadow: Carto @grass */
export const grass = 7;

/** park: Carto @park */
export const park = 8;

/** playground, recreation ground, stadium, sports center: Carto @leisure, lighten(@park, 5%) */
export const leisure = 9;

/** pitch and track: Carto @pitch */
export const pitch = 10;

/** forest, wood, swamp: Carto @forest */
export const forest = 11;

/** orchard, vineyard, plant nursery: Carto @orchard */
export const orchard = 12;

/** allotments: Carto @allotments */
export const allotments = 13;

/** farmland and greenhouses: Carto @farmland */
export const farmland = 14;

/** farmyard: Carto @farmyard */
export const farmyard = 15;

/** campsite and golf course: Carto @campsite */
export const campsite = 16;

/** cemetery and grave yard: Carto @cemetery */
export const cemetery = 17;

/** residential: Carto @residential */
export const residential = 18;

/** garages: Carto @garages */
export const garages = 19;

/** retail: Carto @retail */
export const retail = 20;

/** commercial: Carto @commercial */
export const commercial = 21;

/** industrial, railway, works: Carto @industrial */
export const industrial = 22;

/** power plant and substation: Carto @power, darken(@industrial, 5%) */
export const power = 23;

/** landfill: Carto's landuse_landfill, a value with no name */
export const landfill = 24;

/** quarry: Carto @quarry */
export const quarry = 25;

/** school, college, university, hospital, clinic: Carto @societal_amenities */
export const societal_amenities = 26;

/** a religious site: Carto @place_of_worship */
export const place_of_worship = 27;

/** aerodrome, bus station, ferry terminal: Carto @transportation-area */
export const transportation_area = 28;

/** parking: Carto @parking */
export const parking = 29;

/** apron: Carto @apron */
export const apron = 30;

// ---- water ----

/** glacier: Carto @glacier, drawn with the water areas */
export const glacier = 31;

/** sea, lake, river, canal, any water: Carto @water-color */
export const water = 32;

// ---- water_features ----

/** dam: Carto @dam */
export const dam = 33;

/** breakwater, groyne, weir: Carto @breakwater-color, @weir-line */
export const breakwater = 34;

// ---- buildings ----

/** a building: Carto @building-fill */
export const building = 35;

/** a place of worship, a terminal, a station: Carto @building-major-fill at z16 and finer, as a cell is z16 */
export const building_major = 36;

// ---- roads ----

/** footway, steps, path: Carto @footway-fill, @steps-fill */
export const footway = 37;

/** cycleway: Carto @cycleway-fill */
export const cycleway = 38;

/** bridleway: Carto @bridleway-fill */
export const bridleway = 39;

/** track, an agricultural or forest way: Carto @track-fill */
export const track = 40;

/** pedestrian street: Carto @pedestrian-fill */
export const pedestrian = 41;

/** living street: Carto @living-street-fill */
export const living_street = 42;

/** tertiary, residential, unclassified and service, told apart by width: Carto @tertiary-fill, @residential-fill, @service-fill */
export const residential_road = 43;

/** a road of no known class: Carto @road-fill */
export const road = 44;

/** secondary: Carto @secondary-fill */
export const secondary = 45;

/** primary: Carto @primary-fill */
export const primary = 46;

/** trunk: Carto @trunk-fill */
export const trunk = 47;

/** motorway: Carto @motorway-fill */
export const motorway = 48;

/** platform: Carto @platform-fill */
export const platform = 49;

/** tram: Carto's railway_tram, a value with no name */
export const tram = 50;

/** light rail, narrow gauge, funicular: Carto's railway_light_rail, a value with no name */
export const light_rail = 51;

/** subway: Carto's railway_subway, a value with no name */
export const subway = 52;

/** monorail: Carto's railway_monorail, a value with no name */
export const monorail = 53;

/** rail: Carto's railway_rail, a value with no name */
export const rail = 54;

// ---- aeroways ----

/** runway, taxiway, helipad: Carto @aeroway-fill, @runway-fill, @taxiway-fill, @helipad-fill */
export const aeroway = 55;

// ---- underground ----

/** under a face: Minecraft Beta's filler, dirt; Carto paints nothing under the ground, and no bake writes it */
export const soil = 64;

/** under sand's filler: Minecraft Beta's sandstone */
export const sandstone = 65;

/** under the filler: Minecraft Beta's stone */
export const stone = 66;
