// pxl-content/cube.spec.rs
//
// GENERATED from spec/spec/asset/cube.json
//
// One value space: a ground cell is a byte, a Carto palette entry numbered in the
// order Carto paints, and an object's value a u16 whose byte range is
// that palette and whose range past it is what only an object can be.

// Names as the spec spells them, so one record has one name on every side.
#![allow(
    non_camel_case_types,
    non_upper_case_globals,
    reason = "generated from the spec"
)]

pub mod sentinel {
    /// Nothing is painted: the ground before a bake writes it, and an object cell no leaf holds.
    pub const void: u8 = 0x00;
    /// Bedrock, where the world holds no more: the floor under the ground and the wall just inside the service area (`pxl-spec limit.rs`). A cell past the area or the coverage, which no source here describes, reads as it and is drawn as nothing, and so is a byte no bake wrote. The value is 0xFF so that memory filled with 0xFF reads as unknown. Nothing is placed on it or taken from it.
    pub const unknown: u8 = 0xff;
}

// The ground: one byte a cell, a palette entry from OpenStreetMap Carto, numbered in
// the order Carto paints.
// `transport` marks a way: decor faces along it and no pier stands on it.
// Whatever the palette entry, a cell's top is the height sampled from the
// elevation; no entry raises or lowers it. The byte's values are these: 1 to 63
// are Carto's palette entries, in its paint order; from 64 on are pixelet's own,
// the underground, of which Carto paints nothing; 0 and 255 are the sentinels.
pub mod ground {

    // ---- landcover ----

    /// Land no feature covers, and what Carto draws no fill for, takes Carto's background, @land-color.
    pub const land: u8 = 1;
    /// Bare rock, scree and shingle take Carto's @bare_ground.
    pub const bare_ground: u8 = 2;
    /// Sand takes Carto's @sand.
    pub const sand: u8 = 3;
    /// Beach and shoal take Carto's @beach.
    pub const beach: u8 = 4;
    /// Heath and bog take Carto's @heath.
    pub const heath: u8 = 5;
    /// Scrub and mangrove take Carto's @scrub.
    pub const scrub: u8 = 6;
    /// Grass, meadow, garden, village green, marsh and wet meadow take Carto's @grass.
    pub const grass: u8 = 7;
    /// A park takes Carto's @park.
    pub const park: u8 = 8;
    /// A playground, recreation ground, stadium or sports center takes Carto's @leisure, lighten(@park, 5%).
    pub const leisure: u8 = 9;
    /// A pitch or a track takes Carto's @pitch.
    pub const pitch: u8 = 10;
    /// Forest, wood and swamp take Carto's @forest.
    pub const forest: u8 = 11;
    /// An orchard, vineyard or plant nursery takes Carto's @orchard.
    pub const orchard: u8 = 12;
    /// Allotments take Carto's @allotments.
    pub const allotments: u8 = 13;
    /// Farmland and greenhouses take Carto's @farmland.
    pub const farmland: u8 = 14;
    /// A farmyard takes Carto's @farmyard.
    pub const farmyard: u8 = 15;
    /// A campsite or golf course takes Carto's @campsite.
    pub const campsite: u8 = 16;
    /// A cemetery or grave yard takes Carto's @cemetery.
    pub const cemetery: u8 = 17;
    /// Residential land takes Carto's @residential.
    pub const residential: u8 = 18;
    /// Garages take Carto's @garages.
    pub const garages: u8 = 19;
    /// Retail land takes Carto's @retail.
    pub const retail: u8 = 20;
    /// Commercial land takes Carto's @commercial.
    pub const commercial: u8 = 21;
    /// Industrial land, railway land and works take Carto's @industrial.
    pub const industrial: u8 = 22;
    /// A power plant or substation takes Carto's @power, darken(@industrial, 5%).
    pub const power: u8 = 23;
    /// Landfill takes Carto's landuse_landfill, a value with no name.
    pub const landfill: u8 = 24;
    /// A quarry takes Carto's @quarry.
    pub const quarry: u8 = 25;
    /// A school, college, university, hospital or clinic takes Carto's @societal_amenities.
    pub const societal_amenities: u8 = 26;
    /// A religious site takes Carto's @place_of_worship.
    pub const place_of_worship: u8 = 27;
    /// An aerodrome, bus station or ferry terminal takes Carto's @transportation-area.
    pub const transportation_area: u8 = 28;
    /// Parking takes Carto's @parking.
    pub const parking: u8 = 29;
    /// An apron takes Carto's @apron.
    pub const apron: u8 = 30;

    // ---- water ----

    /// A glacier takes Carto's @glacier; Carto draws it with the water areas.
    pub const glacier: u8 = 31;
    /// Sea, lake, river, canal and any other water take Carto's @water-color.
    pub const water: u8 = 32;

    // ---- water_features ----

    /// A dam takes Carto's @dam.
    pub const dam: u8 = 33;
    /// A breakwater, groyne or weir takes Carto's @breakwater-color or @weir-line.
    pub const breakwater: u8 = 34;

    // ---- buildings ----

    /// A building takes Carto's @building-fill.
    pub const building: u8 = 35;
    /// A place of worship, a terminal or a station takes Carto's @building-major-fill at z16 and finer, as a cell is z16.
    pub const building_major: u8 = 36;

    // ---- roads ----

    /// Footways, steps and paths take Carto's @footway-fill or @steps-fill.
    pub const footway: u8 = 37;
    /// A cycleway takes Carto's @cycleway-fill.
    pub const cycleway: u8 = 38;
    /// A bridleway takes Carto's @bridleway-fill.
    pub const bridleway: u8 = 39;
    /// A track, an agricultural or forest way, takes Carto's @track-fill.
    pub const track: u8 = 40;
    /// A pedestrian street takes Carto's @pedestrian-fill.
    pub const pedestrian: u8 = 41;
    /// A living street takes Carto's @living-street-fill.
    pub const living_street: u8 = 42;
    /// A tertiary, residential, unclassified or service road takes Carto's @tertiary-fill, @residential-fill or @service-fill, told apart by width.
    pub const residential_road: u8 = 43;
    /// A road of no known class takes Carto's @road-fill.
    pub const road: u8 = 44;
    /// A secondary road takes Carto's @secondary-fill.
    pub const secondary: u8 = 45;
    /// A primary road takes Carto's @primary-fill.
    pub const primary: u8 = 46;
    /// A trunk road takes Carto's @trunk-fill.
    pub const trunk: u8 = 47;
    /// A motorway takes Carto's @motorway-fill.
    pub const motorway: u8 = 48;
    /// A platform takes Carto's @platform-fill.
    pub const platform: u8 = 49;
    /// A tram takes Carto's railway_tram, a value with no name.
    pub const tram: u8 = 50;
    /// Light rail, narrow gauge and funicular take Carto's railway_light_rail, a value with no name.
    pub const light_rail: u8 = 51;
    /// A subway takes Carto's railway_subway, a value with no name.
    pub const subway: u8 = 52;
    /// A monorail takes Carto's railway_monorail, a value with no name.
    pub const monorail: u8 = 53;
    /// A railway takes Carto's railway_rail, a value with no name.
    pub const rail: u8 = 54;

    // ---- aeroways ----

    /// A runway, taxiway or helipad takes Carto's @aeroway-fill, @runway-fill, @taxiway-fill or @helipad-fill.
    pub const aeroway: u8 = 55;

    // ---- underground ----

    /// What lies under a face: Minecraft Beta's filler, dirt. Carto paints nothing under the ground, and no bake writes it.
    pub const soil: u8 = 64;
    /// What lies under sand's filler: Minecraft Beta's sandstone.
    pub const sandstone: u8 = 65;
    /// What lies under the filler: Minecraft Beta's stone.
    pub const stone: u8 = 66;

    pub const ENTRIES: &[(&str, u8)] = &[
        ("land", land),
        ("bare_ground", bare_ground),
        ("sand", sand),
        ("beach", beach),
        ("heath", heath),
        ("scrub", scrub),
        ("grass", grass),
        ("park", park),
        ("leisure", leisure),
        ("pitch", pitch),
        ("forest", forest),
        ("orchard", orchard),
        ("allotments", allotments),
        ("farmland", farmland),
        ("farmyard", farmyard),
        ("campsite", campsite),
        ("cemetery", cemetery),
        ("residential", residential),
        ("garages", garages),
        ("retail", retail),
        ("commercial", commercial),
        ("industrial", industrial),
        ("power", power),
        ("landfill", landfill),
        ("quarry", quarry),
        ("societal_amenities", societal_amenities),
        ("place_of_worship", place_of_worship),
        ("transportation_area", transportation_area),
        ("parking", parking),
        ("apron", apron),
        ("glacier", glacier),
        ("water", water),
        ("dam", dam),
        ("breakwater", breakwater),
        ("building", building),
        ("building_major", building_major),
        ("footway", footway),
        ("cycleway", cycleway),
        ("bridleway", bridleway),
        ("track", track),
        ("pedestrian", pedestrian),
        ("living_street", living_street),
        ("residential_road", residential_road),
        ("road", road),
        ("secondary", secondary),
        ("primary", primary),
        ("trunk", trunk),
        ("motorway", motorway),
        ("platform", platform),
        ("tram", tram),
        ("light_rail", light_rail),
        ("subway", subway),
        ("monorail", monorail),
        ("rail", rail),
        ("aeroway", aeroway),
        ("soil", soil),
        ("sandstone", sandstone),
        ("stone", stone),
    ];

    /// What a cell nothing painted shows.
    pub const BASE: u8 = land;
}

// What only an object can be: past a byte, so no ground cell holds it. An
// object's value is a u16 whose byte range is the ground's palette.
pub mod object {
    /// Explicit air: the leaf stays and erases the layers under it.
    pub const air: u16 = 256;

    // ---- colors ----

    pub const white: u16 = 257;
    pub const grey: u16 = 258;
    pub const dark_grey: u16 = 259;
    pub const black: u16 = 260;
    pub const red: u16 = 261;
    pub const orange: u16 = 262;
    pub const brown: u16 = 263;
    pub const yellow: u16 = 264;
    pub const green: u16 = 265;
    pub const dark_green: u16 = 266;
    pub const cyan: u16 = 267;
    pub const blue: u16 = 268;
    pub const violet: u16 = 269;
    pub const magenta: u16 = 270;
    pub const pink: u16 = 271;
}

/// A palette entry, by its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Ground {
    land = 1,
    bare_ground = 2,
    sand = 3,
    beach = 4,
    heath = 5,
    scrub = 6,
    grass = 7,
    park = 8,
    leisure = 9,
    pitch = 10,
    forest = 11,
    orchard = 12,
    allotments = 13,
    farmland = 14,
    farmyard = 15,
    campsite = 16,
    cemetery = 17,
    residential = 18,
    garages = 19,
    retail = 20,
    commercial = 21,
    industrial = 22,
    power = 23,
    landfill = 24,
    quarry = 25,
    societal_amenities = 26,
    place_of_worship = 27,
    transportation_area = 28,
    parking = 29,
    apron = 30,
    glacier = 31,
    water = 32,
    dam = 33,
    breakwater = 34,
    building = 35,
    building_major = 36,
    footway = 37,
    cycleway = 38,
    bridleway = 39,
    track = 40,
    pedestrian = 41,
    living_street = 42,
    residential_road = 43,
    road = 44,
    secondary = 45,
    primary = 46,
    trunk = 47,
    motorway = 48,
    platform = 49,
    tram = 50,
    light_rail = 51,
    subway = 52,
    monorail = 53,
    rail = 54,
    aeroway = 55,
    soil = 64,
    sandstone = 65,
    stone = 66,
}

impl Ground {
    /// Every palette entry, in paint order.
    pub const ALL: [Ground; 58] = [
        Ground::land,
        Ground::bare_ground,
        Ground::sand,
        Ground::beach,
        Ground::heath,
        Ground::scrub,
        Ground::grass,
        Ground::park,
        Ground::leisure,
        Ground::pitch,
        Ground::forest,
        Ground::orchard,
        Ground::allotments,
        Ground::farmland,
        Ground::farmyard,
        Ground::campsite,
        Ground::cemetery,
        Ground::residential,
        Ground::garages,
        Ground::retail,
        Ground::commercial,
        Ground::industrial,
        Ground::power,
        Ground::landfill,
        Ground::quarry,
        Ground::societal_amenities,
        Ground::place_of_worship,
        Ground::transportation_area,
        Ground::parking,
        Ground::apron,
        Ground::glacier,
        Ground::water,
        Ground::dam,
        Ground::breakwater,
        Ground::building,
        Ground::building_major,
        Ground::footway,
        Ground::cycleway,
        Ground::bridleway,
        Ground::track,
        Ground::pedestrian,
        Ground::living_street,
        Ground::residential_road,
        Ground::road,
        Ground::secondary,
        Ground::primary,
        Ground::trunk,
        Ground::motorway,
        Ground::platform,
        Ground::tram,
        Ground::light_rail,
        Ground::subway,
        Ground::monorail,
        Ground::rail,
        Ground::aeroway,
        Ground::soil,
        Ground::sandstone,
        Ground::stone,
    ];

    /// The palette entry a byte names; none for a sentinel or a number the palette does not give out.
    pub const fn of(val: u8) -> Option<Ground> {
        match val {
            1 => Some(Ground::land),
            2 => Some(Ground::bare_ground),
            3 => Some(Ground::sand),
            4 => Some(Ground::beach),
            5 => Some(Ground::heath),
            6 => Some(Ground::scrub),
            7 => Some(Ground::grass),
            8 => Some(Ground::park),
            9 => Some(Ground::leisure),
            10 => Some(Ground::pitch),
            11 => Some(Ground::forest),
            12 => Some(Ground::orchard),
            13 => Some(Ground::allotments),
            14 => Some(Ground::farmland),
            15 => Some(Ground::farmyard),
            16 => Some(Ground::campsite),
            17 => Some(Ground::cemetery),
            18 => Some(Ground::residential),
            19 => Some(Ground::garages),
            20 => Some(Ground::retail),
            21 => Some(Ground::commercial),
            22 => Some(Ground::industrial),
            23 => Some(Ground::power),
            24 => Some(Ground::landfill),
            25 => Some(Ground::quarry),
            26 => Some(Ground::societal_amenities),
            27 => Some(Ground::place_of_worship),
            28 => Some(Ground::transportation_area),
            29 => Some(Ground::parking),
            30 => Some(Ground::apron),
            31 => Some(Ground::glacier),
            32 => Some(Ground::water),
            33 => Some(Ground::dam),
            34 => Some(Ground::breakwater),
            35 => Some(Ground::building),
            36 => Some(Ground::building_major),
            37 => Some(Ground::footway),
            38 => Some(Ground::cycleway),
            39 => Some(Ground::bridleway),
            40 => Some(Ground::track),
            41 => Some(Ground::pedestrian),
            42 => Some(Ground::living_street),
            43 => Some(Ground::residential_road),
            44 => Some(Ground::road),
            45 => Some(Ground::secondary),
            46 => Some(Ground::primary),
            47 => Some(Ground::trunk),
            48 => Some(Ground::motorway),
            49 => Some(Ground::platform),
            50 => Some(Ground::tram),
            51 => Some(Ground::light_rail),
            52 => Some(Ground::subway),
            53 => Some(Ground::monorail),
            54 => Some(Ground::rail),
            55 => Some(Ground::aeroway),
            64 => Some(Ground::soil),
            65 => Some(Ground::sandstone),
            66 => Some(Ground::stone),
            _ => None,
        }
    }

    /// Whether a palette entry read into a word, as a GPU has it with no byte, is one `of`
    /// names.
    pub const fn grounds(val: u32) -> bool {
        matches!(val, 1..=55 | 64..=66)
    }

    /// A way: decor faces along it and no pier stands on it.
    pub const fn transport(self) -> bool {
        Ground::transports(self as u32)
    }

    /// Whether a palette entry read into a word is a way (`transport`).
    pub const fn transports(val: u32) -> bool {
        matches!(val, 37..=55)
    }
}

/// One of the object's colors, by its number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Object {
    white = 257,
    grey = 258,
    dark_grey = 259,
    black = 260,
    red = 261,
    orange = 262,
    brown = 263,
    yellow = 264,
    green = 265,
    dark_green = 266,
    cyan = 267,
    blue = 268,
    violet = 269,
    magenta = 270,
    pink = 271,
}

impl Object {
    pub const ALL: [Object; 15] = [
        Object::white,
        Object::grey,
        Object::dark_grey,
        Object::black,
        Object::red,
        Object::orange,
        Object::brown,
        Object::yellow,
        Object::green,
        Object::dark_green,
        Object::cyan,
        Object::blue,
        Object::violet,
        Object::magenta,
        Object::pink,
    ];

    pub const fn of(val: u16) -> Option<Object> {
        match val {
            257 => Some(Object::white),
            258 => Some(Object::grey),
            259 => Some(Object::dark_grey),
            260 => Some(Object::black),
            261 => Some(Object::red),
            262 => Some(Object::orange),
            263 => Some(Object::brown),
            264 => Some(Object::yellow),
            265 => Some(Object::green),
            266 => Some(Object::dark_green),
            267 => Some(Object::cyan),
            268 => Some(Object::blue),
            269 => Some(Object::violet),
            270 => Some(Object::magenta),
            271 => Some(Object::pink),
            _ => None,
        }
    }

    /// Whether a value read into a word, as a GPU has it, is one `of` names.
    pub const fn objects(val: u32) -> bool {
        matches!(val, 257..=271)
    }
}
