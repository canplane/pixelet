// pxl-spec/asset/cube.spec.rs
//
// GENERATED from spec/spec/asset/cube.json (xpute-gen data.rs)

/// The texture pack, the file the cube kind's palette entry is drawn from: one image
/// of every tile under asset/content/cube/, which the asset ingest composes
/// and ships as the table's file at this path, with which tile each cube
/// wears on which face (pxl-tool bin/asset.rs, ingest.json). In Rust it is
/// the slot the catalog fills with it.
pub const PACK: u16 = 0;

/// What a cell shows when nothing painted it: a cell with nothing on it is
/// still land, as Carto's background is.
pub const BASE: Ground = Ground::Land;

/// The value space's own: what no palette entry or color is (pxl-spec value.rs).
#[derive(Clone, Copy, Debug)]
pub struct SentinelRecord {
    pub name: &'static str,
}

pub const SENTINEL: [SentinelRecord; 3] = [
    SentinelRecord { name: "void" },
    SentinelRecord { name: "unknown" },
    SentinelRecord { name: "air" },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum Sentinel {
    /// Nothing is painted: the ground before a bake writes it, and an object cell no leaf holds.
    Void = 0,
    /// Bedrock, where the world holds no more: the floor under the ground and the wall just inside the service area (`pxl-content strata.rs`). A cell past the area or the coverage, which no source here describes, reads as it. It is solid and drawn as the bedrock's layer (pxl-render paint/palette.rs `SENTINELS`). Nothing is placed on it or taken from it.
    Unknown = 255,
    /// Explicit air: the leaf stays and erases the layers under it.
    Air = 256,
}

impl Sentinel {
    /// Every member, in its source's own order.
    pub const ALL: [Sentinel; 3] = [Sentinel::Void, Sentinel::Unknown, Sentinel::Air];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u16) -> Option<Sentinel> {
        match val {
            0 => Some(Sentinel::Void),
            255 => Some(Sentinel::Unknown),
            256 => Some(Sentinel::Air),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Sentinel::Void => "void",
            Sentinel::Unknown => "unknown",
            Sentinel::Air => "air",
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

pub const GROUND: [GroundRecord; 58] = [
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
    Land = 1,
    /// Bare rock, scree and shingle take Carto's @bare_ground.
    BareGround = 2,
    /// Sand takes Carto's @sand.
    Sand = 3,
    /// Beach and shoal take Carto's @beach.
    Beach = 4,
    /// Heath and bog take Carto's @heath.
    Heath = 5,
    /// Scrub and mangrove take Carto's @scrub.
    Scrub = 6,
    /// Grass, meadow, garden, village green, marsh and wet meadow take Carto's @grass.
    Grass = 7,
    /// A park takes Carto's @park.
    Park = 8,
    /// A playground, recreation ground, stadium or sports center takes Carto's @leisure, lighten(@park, 5%).
    Leisure = 9,
    /// A pitch or a track takes Carto's @pitch.
    Pitch = 10,
    /// Forest, wood and swamp take Carto's @forest.
    Forest = 11,
    /// An orchard, vineyard or plant nursery takes Carto's @orchard.
    Orchard = 12,
    /// Allotments take Carto's @allotments.
    Allotments = 13,
    /// Farmland and greenhouses take Carto's @farmland.
    Farmland = 14,
    /// A farmyard takes Carto's @farmyard.
    Farmyard = 15,
    /// A campsite or golf course takes Carto's @campsite.
    Campsite = 16,
    /// A cemetery or grave yard takes Carto's @cemetery.
    Cemetery = 17,
    /// Residential land takes Carto's @residential.
    Residential = 18,
    /// Garages take Carto's @garages.
    Garages = 19,
    /// Retail land takes Carto's @retail.
    Retail = 20,
    /// Commercial land takes Carto's @commercial.
    Commercial = 21,
    /// Industrial land, railway land and works take Carto's @industrial.
    Industrial = 22,
    /// A power plant or substation takes Carto's @power, darken(@industrial, 5%).
    Power = 23,
    /// Landfill takes Carto's landuse_landfill, a value with no name.
    Landfill = 24,
    /// A quarry takes Carto's @quarry.
    Quarry = 25,
    /// A school, college, university, hospital or clinic takes Carto's @societal_amenities.
    SocietalAmenities = 26,
    /// A religious site takes Carto's @place_of_worship.
    PlaceOfWorship = 27,
    /// An aerodrome, bus station or ferry terminal takes Carto's @transportation-area.
    TransportationArea = 28,
    /// Parking takes Carto's @parking.
    Parking = 29,
    /// An apron takes Carto's @apron.
    Apron = 30,
    /// A glacier takes Carto's @glacier; Carto draws it with the water areas.
    Glacier = 31,
    /// Sea, lake, river, canal and any other water take Carto's @water-color.
    Water = 32,
    /// A dam takes Carto's @dam.
    Dam = 33,
    /// A breakwater, groyne or weir takes Carto's @breakwater-color or @weir-line.
    Breakwater = 34,
    /// A building takes Carto's @building-fill.
    Building = 35,
    /// A place of worship, a terminal or a station takes Carto's @building-major-fill at z16 and finer, as a cell is z16.
    BuildingMajor = 36,
    /// Footways, steps and paths take Carto's @footway-fill or @steps-fill.
    Footway = 37,
    /// A cycleway takes Carto's @cycleway-fill.
    Cycleway = 38,
    /// A bridleway takes Carto's @bridleway-fill.
    Bridleway = 39,
    /// A track, an agricultural or forest way, takes Carto's @track-fill.
    Track = 40,
    /// A pedestrian street takes Carto's @pedestrian-fill.
    Pedestrian = 41,
    /// A living street takes Carto's @living-street-fill.
    LivingStreet = 42,
    /// A tertiary, residential, unclassified or service road takes Carto's @tertiary-fill, @residential-fill or @service-fill, told apart by width.
    ResidentialRoad = 43,
    /// A road of no known class takes Carto's @road-fill.
    Road = 44,
    /// A secondary road takes Carto's @secondary-fill.
    Secondary = 45,
    /// A primary road takes Carto's @primary-fill.
    Primary = 46,
    /// A trunk road takes Carto's @trunk-fill.
    Trunk = 47,
    /// A motorway takes Carto's @motorway-fill.
    Motorway = 48,
    /// A platform takes Carto's @platform-fill.
    Platform = 49,
    /// A tram takes Carto's railway_tram, a value with no name.
    Tram = 50,
    /// Light rail, narrow gauge and funicular take Carto's railway_light_rail, a value with no name.
    LightRail = 51,
    /// A subway takes Carto's railway_subway, a value with no name.
    Subway = 52,
    /// A monorail takes Carto's railway_monorail, a value with no name.
    Monorail = 53,
    /// A railway takes Carto's railway_rail, a value with no name.
    Rail = 54,
    /// A runway, taxiway or helipad takes Carto's @aeroway-fill, @runway-fill, @taxiway-fill or @helipad-fill.
    Aeroway = 55,
    /// What lies under a face: Minecraft Beta's filler, dirt. Carto paints nothing under the ground, and no bake writes it.
    Soil = 64,
    /// What lies under sand's filler: Minecraft Beta's sandstone.
    Sandstone = 65,
    /// What lies under the filler: Minecraft Beta's stone.
    Stone = 66,
}

impl Ground {
    /// Every member, in its source's own order.
    pub const ALL: [Ground; 58] = [
        Ground::Land,
        Ground::BareGround,
        Ground::Sand,
        Ground::Beach,
        Ground::Heath,
        Ground::Scrub,
        Ground::Grass,
        Ground::Park,
        Ground::Leisure,
        Ground::Pitch,
        Ground::Forest,
        Ground::Orchard,
        Ground::Allotments,
        Ground::Farmland,
        Ground::Farmyard,
        Ground::Campsite,
        Ground::Cemetery,
        Ground::Residential,
        Ground::Garages,
        Ground::Retail,
        Ground::Commercial,
        Ground::Industrial,
        Ground::Power,
        Ground::Landfill,
        Ground::Quarry,
        Ground::SocietalAmenities,
        Ground::PlaceOfWorship,
        Ground::TransportationArea,
        Ground::Parking,
        Ground::Apron,
        Ground::Glacier,
        Ground::Water,
        Ground::Dam,
        Ground::Breakwater,
        Ground::Building,
        Ground::BuildingMajor,
        Ground::Footway,
        Ground::Cycleway,
        Ground::Bridleway,
        Ground::Track,
        Ground::Pedestrian,
        Ground::LivingStreet,
        Ground::ResidentialRoad,
        Ground::Road,
        Ground::Secondary,
        Ground::Primary,
        Ground::Trunk,
        Ground::Motorway,
        Ground::Platform,
        Ground::Tram,
        Ground::LightRail,
        Ground::Subway,
        Ground::Monorail,
        Ground::Rail,
        Ground::Aeroway,
        Ground::Soil,
        Ground::Sandstone,
        Ground::Stone,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Ground> {
        match val {
            1 => Some(Ground::Land),
            2 => Some(Ground::BareGround),
            3 => Some(Ground::Sand),
            4 => Some(Ground::Beach),
            5 => Some(Ground::Heath),
            6 => Some(Ground::Scrub),
            7 => Some(Ground::Grass),
            8 => Some(Ground::Park),
            9 => Some(Ground::Leisure),
            10 => Some(Ground::Pitch),
            11 => Some(Ground::Forest),
            12 => Some(Ground::Orchard),
            13 => Some(Ground::Allotments),
            14 => Some(Ground::Farmland),
            15 => Some(Ground::Farmyard),
            16 => Some(Ground::Campsite),
            17 => Some(Ground::Cemetery),
            18 => Some(Ground::Residential),
            19 => Some(Ground::Garages),
            20 => Some(Ground::Retail),
            21 => Some(Ground::Commercial),
            22 => Some(Ground::Industrial),
            23 => Some(Ground::Power),
            24 => Some(Ground::Landfill),
            25 => Some(Ground::Quarry),
            26 => Some(Ground::SocietalAmenities),
            27 => Some(Ground::PlaceOfWorship),
            28 => Some(Ground::TransportationArea),
            29 => Some(Ground::Parking),
            30 => Some(Ground::Apron),
            31 => Some(Ground::Glacier),
            32 => Some(Ground::Water),
            33 => Some(Ground::Dam),
            34 => Some(Ground::Breakwater),
            35 => Some(Ground::Building),
            36 => Some(Ground::BuildingMajor),
            37 => Some(Ground::Footway),
            38 => Some(Ground::Cycleway),
            39 => Some(Ground::Bridleway),
            40 => Some(Ground::Track),
            41 => Some(Ground::Pedestrian),
            42 => Some(Ground::LivingStreet),
            43 => Some(Ground::ResidentialRoad),
            44 => Some(Ground::Road),
            45 => Some(Ground::Secondary),
            46 => Some(Ground::Primary),
            47 => Some(Ground::Trunk),
            48 => Some(Ground::Motorway),
            49 => Some(Ground::Platform),
            50 => Some(Ground::Tram),
            51 => Some(Ground::LightRail),
            52 => Some(Ground::Subway),
            53 => Some(Ground::Monorail),
            54 => Some(Ground::Rail),
            55 => Some(Ground::Aeroway),
            64 => Some(Ground::Soil),
            65 => Some(Ground::Sandstone),
            66 => Some(Ground::Stone),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Ground::Land => "land",
            Ground::BareGround => "bare_ground",
            Ground::Sand => "sand",
            Ground::Beach => "beach",
            Ground::Heath => "heath",
            Ground::Scrub => "scrub",
            Ground::Grass => "grass",
            Ground::Park => "park",
            Ground::Leisure => "leisure",
            Ground::Pitch => "pitch",
            Ground::Forest => "forest",
            Ground::Orchard => "orchard",
            Ground::Allotments => "allotments",
            Ground::Farmland => "farmland",
            Ground::Farmyard => "farmyard",
            Ground::Campsite => "campsite",
            Ground::Cemetery => "cemetery",
            Ground::Residential => "residential",
            Ground::Garages => "garages",
            Ground::Retail => "retail",
            Ground::Commercial => "commercial",
            Ground::Industrial => "industrial",
            Ground::Power => "power",
            Ground::Landfill => "landfill",
            Ground::Quarry => "quarry",
            Ground::SocietalAmenities => "societal_amenities",
            Ground::PlaceOfWorship => "place_of_worship",
            Ground::TransportationArea => "transportation_area",
            Ground::Parking => "parking",
            Ground::Apron => "apron",
            Ground::Glacier => "glacier",
            Ground::Water => "water",
            Ground::Dam => "dam",
            Ground::Breakwater => "breakwater",
            Ground::Building => "building",
            Ground::BuildingMajor => "building_major",
            Ground::Footway => "footway",
            Ground::Cycleway => "cycleway",
            Ground::Bridleway => "bridleway",
            Ground::Track => "track",
            Ground::Pedestrian => "pedestrian",
            Ground::LivingStreet => "living_street",
            Ground::ResidentialRoad => "residential_road",
            Ground::Road => "road",
            Ground::Secondary => "secondary",
            Ground::Primary => "primary",
            Ground::Trunk => "trunk",
            Ground::Motorway => "motorway",
            Ground::Platform => "platform",
            Ground::Tram => "tram",
            Ground::LightRail => "light_rail",
            Ground::Subway => "subway",
            Ground::Monorail => "monorail",
            Ground::Rail => "rail",
            Ground::Aeroway => "aeroway",
            Ground::Soil => "soil",
            Ground::Sandstone => "sandstone",
            Ground::Stone => "stone",
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

pub const OBJECT: [ObjectRecord; 15] = [
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
    White = 257,
    Grey = 258,
    DarkGrey = 259,
    Black = 260,
    Red = 261,
    Orange = 262,
    Brown = 263,
    Yellow = 264,
    Green = 265,
    DarkGreen = 266,
    Cyan = 267,
    Blue = 268,
    Violet = 269,
    Magenta = 270,
    Pink = 271,
}

impl Object {
    /// Every member, in its source's own order.
    pub const ALL: [Object; 15] = [
        Object::White,
        Object::Grey,
        Object::DarkGrey,
        Object::Black,
        Object::Red,
        Object::Orange,
        Object::Brown,
        Object::Yellow,
        Object::Green,
        Object::DarkGreen,
        Object::Cyan,
        Object::Blue,
        Object::Violet,
        Object::Magenta,
        Object::Pink,
    ];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u16) -> Option<Object> {
        match val {
            257 => Some(Object::White),
            258 => Some(Object::Grey),
            259 => Some(Object::DarkGrey),
            260 => Some(Object::Black),
            261 => Some(Object::Red),
            262 => Some(Object::Orange),
            263 => Some(Object::Brown),
            264 => Some(Object::Yellow),
            265 => Some(Object::Green),
            266 => Some(Object::DarkGreen),
            267 => Some(Object::Cyan),
            268 => Some(Object::Blue),
            269 => Some(Object::Violet),
            270 => Some(Object::Magenta),
            271 => Some(Object::Pink),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Object::White => "white",
            Object::Grey => "grey",
            Object::DarkGrey => "dark_grey",
            Object::Black => "black",
            Object::Red => "red",
            Object::Orange => "orange",
            Object::Brown => "brown",
            Object::Yellow => "yellow",
            Object::Green => "green",
            Object::DarkGreen => "dark_green",
            Object::Cyan => "cyan",
            Object::Blue => "blue",
            Object::Violet => "violet",
            Object::Magenta => "magenta",
            Object::Pink => "pink",
        }
    }
}
