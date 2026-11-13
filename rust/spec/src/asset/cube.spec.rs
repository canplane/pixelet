// pxl-spec/asset/cube.spec.rs
//
// GENERATED from spec/spec/asset/cube.json (xpute-gen data.rs)

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

/// The texture pack, the file the cube kind's palette entry is drawn from: one image
/// of every tile under asset/content/cube/, which the asset ingest composes
/// and ships as the table's file at this path, with which tile each cube
/// wears on which face (pxl-tool bin/asset.rs, ingest.json). In Rust it is
/// the slot the catalog fills with it.
pub const pack: u16 = 0;

/// What a cell shows when nothing painted it: a cell with nothing on it is
/// still land, as Carto's background is.
pub const BASE: Ground = Ground::land;

/// The value space's own: what no palette entry or color is (pxl-spec value.rs).
#[derive(Clone, Copy, Debug)]
pub struct SentinelRecord {
    pub name: &'static str,
}

pub const sentinel: [SentinelRecord; 3] = [
    SentinelRecord { name: "void" },
    SentinelRecord { name: "unknown" },
    SentinelRecord { name: "air" },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Sentinel {
    /// Nothing is painted: the ground before a bake writes it, and an object cell no leaf holds.
    void = 0,
    /// Bedrock, where the world holds no more: the floor under the ground and the wall just inside the service area (`pxl-content strata.rs`). A cell past the area or the coverage, which no source here describes, reads as it and is drawn as nothing, and so is a byte no bake wrote. The value is 0xFF so that memory filled with 0xFF reads as unknown. Nothing is placed on it or taken from it.
    unknown = 255,
    /// Explicit air: the leaf stays and erases the layers under it.
    air = 256,
}

impl Sentinel {
    /// Every member, in its source's own order.
    pub const ALL: [Sentinel; 3] = [Sentinel::void, Sentinel::unknown, Sentinel::air];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u16) -> Option<Sentinel> {
        match val {
            0 => Some(Sentinel::void),
            255 => Some(Sentinel::unknown),
            256 => Some(Sentinel::air),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Sentinel::void => "void",
            Sentinel::unknown => "unknown",
            Sentinel::air => "air",
        }
    }
}

/// The ground: one byte a cell, a palette entry from OpenStreetMap Carto, numbered in
/// the order Carto paints.
/// `transport` marks a way: decor faces along it and no pier stands on it.
/// Whatever the palette entry, a cell's top is the height sampled from the
/// elevation; no entry raises or lowers it. The byte's values are these: 1 to 63
/// are Carto's palette entries, in its paint order; from 64 on are pixelet's own,
/// the underground, of which Carto paints nothing; 0 and 255 are the sentinels.
#[derive(Clone, Copy, Debug)]
pub struct GroundRecord {
    pub name: &'static str,
    pub label: &'static str,
    pub section: &'static str,
    pub transport: bool,
}

pub const ground: [GroundRecord; 58] = [
    GroundRecord {
        name: "land",
        label: "Land",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "bare_ground",
        label: "Bare Ground",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "sand",
        label: "Sand",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "beach",
        label: "Beach",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "heath",
        label: "Heath",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "scrub",
        label: "Scrub",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "grass",
        label: "Grass",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "park",
        label: "Park",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "leisure",
        label: "Leisure",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "pitch",
        label: "Pitch",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "forest",
        label: "Forest",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "orchard",
        label: "Orchard",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "allotments",
        label: "Allotments",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "farmland",
        label: "Farmland",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "farmyard",
        label: "Farmyard",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "campsite",
        label: "Campsite",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "cemetery",
        label: "Cemetery",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "residential",
        label: "Residential",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "garages",
        label: "Garages",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "retail",
        label: "Retail",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "commercial",
        label: "Commercial",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "industrial",
        label: "Industrial",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "power",
        label: "Power",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "landfill",
        label: "Landfill",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "quarry",
        label: "Quarry",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "societal_amenities",
        label: "Societal Amenities",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "place_of_worship",
        label: "Place of Worship",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "transportation_area",
        label: "Transportation Area",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "parking",
        label: "Parking",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "apron",
        label: "Apron",
        section: "landcover",
        transport: false,
    },
    GroundRecord {
        name: "glacier",
        label: "Glacier",
        section: "water",
        transport: false,
    },
    GroundRecord {
        name: "water",
        label: "Water",
        section: "water",
        transport: false,
    },
    GroundRecord {
        name: "dam",
        label: "Dam",
        section: "water_features",
        transport: false,
    },
    GroundRecord {
        name: "breakwater",
        label: "Breakwater",
        section: "water_features",
        transport: false,
    },
    GroundRecord {
        name: "building",
        label: "Building",
        section: "buildings",
        transport: false,
    },
    GroundRecord {
        name: "building_major",
        label: "Major Building",
        section: "buildings",
        transport: false,
    },
    GroundRecord {
        name: "footway",
        label: "Footway",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "cycleway",
        label: "Cycleway",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "bridleway",
        label: "Bridleway",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "track",
        label: "Track",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "pedestrian",
        label: "Pedestrian Street",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "living_street",
        label: "Living Street",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "residential_road",
        label: "Residential Road",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "road",
        label: "Road",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "secondary",
        label: "Secondary Road",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "primary",
        label: "Primary Road",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "trunk",
        label: "Trunk Road",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "motorway",
        label: "Motorway",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "platform",
        label: "Platform",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "tram",
        label: "Tram",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "light_rail",
        label: "Light Rail",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "subway",
        label: "Subway",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "monorail",
        label: "Monorail",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "rail",
        label: "Railway",
        section: "roads",
        transport: true,
    },
    GroundRecord {
        name: "aeroway",
        label: "Runway",
        section: "aeroways",
        transport: true,
    },
    GroundRecord {
        name: "soil",
        label: "Dirt",
        section: "underground",
        transport: false,
    },
    GroundRecord {
        name: "sandstone",
        label: "Sandstone",
        section: "underground",
        transport: false,
    },
    GroundRecord {
        name: "stone",
        label: "Stone",
        section: "underground",
        transport: false,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Ground {
    /// Land no feature covers, and what Carto draws no fill for, takes Carto's background, @land-color.
    land = 1,
    /// Bare rock, scree and shingle take Carto's @bare_ground.
    bare_ground = 2,
    /// Sand takes Carto's @sand.
    sand = 3,
    /// Beach and shoal take Carto's @beach.
    beach = 4,
    /// Heath and bog take Carto's @heath.
    heath = 5,
    /// Scrub and mangrove take Carto's @scrub.
    scrub = 6,
    /// Grass, meadow, garden, village green, marsh and wet meadow take Carto's @grass.
    grass = 7,
    /// A park takes Carto's @park.
    park = 8,
    /// A playground, recreation ground, stadium or sports center takes Carto's @leisure, lighten(@park, 5%).
    leisure = 9,
    /// A pitch or a track takes Carto's @pitch.
    pitch = 10,
    /// Forest, wood and swamp take Carto's @forest.
    forest = 11,
    /// An orchard, vineyard or plant nursery takes Carto's @orchard.
    orchard = 12,
    /// Allotments take Carto's @allotments.
    allotments = 13,
    /// Farmland and greenhouses take Carto's @farmland.
    farmland = 14,
    /// A farmyard takes Carto's @farmyard.
    farmyard = 15,
    /// A campsite or golf course takes Carto's @campsite.
    campsite = 16,
    /// A cemetery or grave yard takes Carto's @cemetery.
    cemetery = 17,
    /// Residential land takes Carto's @residential.
    residential = 18,
    /// Garages take Carto's @garages.
    garages = 19,
    /// Retail land takes Carto's @retail.
    retail = 20,
    /// Commercial land takes Carto's @commercial.
    commercial = 21,
    /// Industrial land, railway land and works take Carto's @industrial.
    industrial = 22,
    /// A power plant or substation takes Carto's @power, darken(@industrial, 5%).
    power = 23,
    /// Landfill takes Carto's landuse_landfill, a value with no name.
    landfill = 24,
    /// A quarry takes Carto's @quarry.
    quarry = 25,
    /// A school, college, university, hospital or clinic takes Carto's @societal_amenities.
    societal_amenities = 26,
    /// A religious site takes Carto's @place_of_worship.
    place_of_worship = 27,
    /// An aerodrome, bus station or ferry terminal takes Carto's @transportation-area.
    transportation_area = 28,
    /// Parking takes Carto's @parking.
    parking = 29,
    /// An apron takes Carto's @apron.
    apron = 30,
    /// A glacier takes Carto's @glacier; Carto draws it with the water areas.
    glacier = 31,
    /// Sea, lake, river, canal and any other water take Carto's @water-color.
    water = 32,
    /// A dam takes Carto's @dam.
    dam = 33,
    /// A breakwater, groyne or weir takes Carto's @breakwater-color or @weir-line.
    breakwater = 34,
    /// A building takes Carto's @building-fill.
    building = 35,
    /// A place of worship, a terminal or a station takes Carto's @building-major-fill at z16 and finer, as a cell is z16.
    building_major = 36,
    /// Footways, steps and paths take Carto's @footway-fill or @steps-fill.
    footway = 37,
    /// A cycleway takes Carto's @cycleway-fill.
    cycleway = 38,
    /// A bridleway takes Carto's @bridleway-fill.
    bridleway = 39,
    /// A track, an agricultural or forest way, takes Carto's @track-fill.
    track = 40,
    /// A pedestrian street takes Carto's @pedestrian-fill.
    pedestrian = 41,
    /// A living street takes Carto's @living-street-fill.
    living_street = 42,
    /// A tertiary, residential, unclassified or service road takes Carto's @tertiary-fill, @residential-fill or @service-fill, told apart by width.
    residential_road = 43,
    /// A road of no known class takes Carto's @road-fill.
    road = 44,
    /// A secondary road takes Carto's @secondary-fill.
    secondary = 45,
    /// A primary road takes Carto's @primary-fill.
    primary = 46,
    /// A trunk road takes Carto's @trunk-fill.
    trunk = 47,
    /// A motorway takes Carto's @motorway-fill.
    motorway = 48,
    /// A platform takes Carto's @platform-fill.
    platform = 49,
    /// A tram takes Carto's railway_tram, a value with no name.
    tram = 50,
    /// Light rail, narrow gauge and funicular take Carto's railway_light_rail, a value with no name.
    light_rail = 51,
    /// A subway takes Carto's railway_subway, a value with no name.
    subway = 52,
    /// A monorail takes Carto's railway_monorail, a value with no name.
    monorail = 53,
    /// A railway takes Carto's railway_rail, a value with no name.
    rail = 54,
    /// A runway, taxiway or helipad takes Carto's @aeroway-fill, @runway-fill, @taxiway-fill or @helipad-fill.
    aeroway = 55,
    /// What lies under a face: Minecraft Beta's filler, dirt. Carto paints nothing under the ground, and no bake writes it.
    soil = 64,
    /// What lies under sand's filler: Minecraft Beta's sandstone.
    sandstone = 65,
    /// What lies under the filler: Minecraft Beta's stone.
    stone = 66,
}

impl Ground {
    /// Every member, in its source's own order.
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

    /// The member `val` is, or none for a number no member has.
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

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Ground::land => "land",
            Ground::bare_ground => "bare_ground",
            Ground::sand => "sand",
            Ground::beach => "beach",
            Ground::heath => "heath",
            Ground::scrub => "scrub",
            Ground::grass => "grass",
            Ground::park => "park",
            Ground::leisure => "leisure",
            Ground::pitch => "pitch",
            Ground::forest => "forest",
            Ground::orchard => "orchard",
            Ground::allotments => "allotments",
            Ground::farmland => "farmland",
            Ground::farmyard => "farmyard",
            Ground::campsite => "campsite",
            Ground::cemetery => "cemetery",
            Ground::residential => "residential",
            Ground::garages => "garages",
            Ground::retail => "retail",
            Ground::commercial => "commercial",
            Ground::industrial => "industrial",
            Ground::power => "power",
            Ground::landfill => "landfill",
            Ground::quarry => "quarry",
            Ground::societal_amenities => "societal_amenities",
            Ground::place_of_worship => "place_of_worship",
            Ground::transportation_area => "transportation_area",
            Ground::parking => "parking",
            Ground::apron => "apron",
            Ground::glacier => "glacier",
            Ground::water => "water",
            Ground::dam => "dam",
            Ground::breakwater => "breakwater",
            Ground::building => "building",
            Ground::building_major => "building_major",
            Ground::footway => "footway",
            Ground::cycleway => "cycleway",
            Ground::bridleway => "bridleway",
            Ground::track => "track",
            Ground::pedestrian => "pedestrian",
            Ground::living_street => "living_street",
            Ground::residential_road => "residential_road",
            Ground::road => "road",
            Ground::secondary => "secondary",
            Ground::primary => "primary",
            Ground::trunk => "trunk",
            Ground::motorway => "motorway",
            Ground::platform => "platform",
            Ground::tram => "tram",
            Ground::light_rail => "light_rail",
            Ground::subway => "subway",
            Ground::monorail => "monorail",
            Ground::rail => "rail",
            Ground::aeroway => "aeroway",
            Ground::soil => "soil",
            Ground::sandstone => "sandstone",
            Ground::stone => "stone",
        }
    }
}

/// What only an object can be: past a byte, so no ground cell holds it. An
/// object's value is a u16 whose byte range is the ground's palette.
#[derive(Clone, Copy, Debug)]
pub struct ObjectRecord {
    pub name: &'static str,
    pub label: &'static str,
    pub section: &'static str,
}

pub const object: [ObjectRecord; 15] = [
    ObjectRecord {
        name: "white",
        label: "White",
        section: "colors",
    },
    ObjectRecord {
        name: "grey",
        label: "Gray",
        section: "colors",
    },
    ObjectRecord {
        name: "dark_grey",
        label: "Dark Gray",
        section: "colors",
    },
    ObjectRecord {
        name: "black",
        label: "Black",
        section: "colors",
    },
    ObjectRecord {
        name: "red",
        label: "Red",
        section: "colors",
    },
    ObjectRecord {
        name: "orange",
        label: "Orange",
        section: "colors",
    },
    ObjectRecord {
        name: "brown",
        label: "Brown",
        section: "colors",
    },
    ObjectRecord {
        name: "yellow",
        label: "Yellow",
        section: "colors",
    },
    ObjectRecord {
        name: "green",
        label: "Green",
        section: "colors",
    },
    ObjectRecord {
        name: "dark_green",
        label: "Dark Green",
        section: "colors",
    },
    ObjectRecord {
        name: "cyan",
        label: "Cyan",
        section: "colors",
    },
    ObjectRecord {
        name: "blue",
        label: "Blue",
        section: "colors",
    },
    ObjectRecord {
        name: "violet",
        label: "Violet",
        section: "colors",
    },
    ObjectRecord {
        name: "magenta",
        label: "Magenta",
        section: "colors",
    },
    ObjectRecord {
        name: "pink",
        label: "Pink",
        section: "colors",
    },
];

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
    /// Every member, in its source's own order.
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

    /// The member `val` is, or none for a number no member has.
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

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Object::white => "white",
            Object::grey => "grey",
            Object::dark_grey => "dark_grey",
            Object::black => "black",
            Object::red => "red",
            Object::orange => "orange",
            Object::brown => "brown",
            Object::yellow => "yellow",
            Object::green => "green",
            Object::dark_green => "dark_green",
            Object::cyan => "cyan",
            Object::blue => "blue",
            Object::violet => "violet",
            Object::magenta => "magenta",
            Object::pink => "pink",
        }
    }
}
