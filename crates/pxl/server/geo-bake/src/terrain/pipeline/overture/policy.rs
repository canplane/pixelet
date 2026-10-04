// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings, Transportation and Base rows to what the ground paints.
//! Segment and base values come from a Gangnam sample; building classes from
//! the published schema (only ~14% of Korean buildings have one).

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::WIDEST;
use pxl_spec::bake::format::GroundChannelKey;
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m, G3857};

use crate::deck::Drawn;
use crate::segment::Segment;
use crate::terrain::pipeline::feature::{Cell, Feature, Joins};
use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, length_of, merged, split_ranged};
use crate::terrain::pipeline::overture::role::Role;
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::terrain::repository::{BaseRow, BuildingRow};
use crate::wkb::wkb_to_geometries;

// ============ Domain knowledge constants ============

/// The transport type a way shows, on the ground or on a deck: its class
/// as OpenMapTiles' transportation layer classes it (MapTiler's schema) —
/// motorway, trunk, primary, secondary, tertiary, minor, service, track and
/// path, rail and transit — one type a class, as neither finer nor coarser
/// than the maps the data is read on.
pub fn segment_type(subtype: &str, class: Option<&str>) -> u8 {
    use cube_types::transport as t;
    let c = class.unwrap_or("");
    if subtype == "rail" {
        return match c {
            "subway" | "light_rail" | "tram" | "monorail" => t::rail_subway,
            _ => t::rail_main,
        };
    }
    match c {
        "motorway" => t::road_motorway,
        "trunk" => t::road_trunk,
        "primary" => t::road_primary,
        "secondary" => t::road_secondary,
        "tertiary" => t::road_tertiary,
        "service" => t::road_service,
        "track" => t::track,
        _ if PEDESTRIAN_CLASSES.contains(&c) => t::pedestrian,
        _ => t::road_default,
    }
}

/// A way's width in metres, the class's or a lane's where it has none: the
/// one width a way is painted on the ground and carried on a deck at.
pub fn segment_width_m(class: &str) -> f64 {
    road_width_by_class(class).unwrap_or(3.5)
}

/// The transport type a way shows, on the ground or on a deck.
pub fn segment_type(subtype: &str, class: Option<&str>) -> u8 {
    let c = class.unwrap_or("");
    if subtype == "rail" {
        if c == "subway" || c == "light_rail" {
            cube_types::transport::rail_subway
        } else {
            cube_types::transport::rail_main
        }
    } else if PEDESTRIAN_CLASSES.contains(&c) {
        cube_types::transport::pedestrian
    } else if MOTOR_CLASSES.contains(&c) {
        cube_types::transport::road_motorway
    } else if c == "primary" {
        cube_types::transport::road_primary
    } else if SECONDARY_CLASSES.contains(&c) {
        cube_types::transport::road_secondary
    } else {
        cube_types::transport::road_default
    }
}

const PEDESTRIAN_CLASSES: [&str; 6] = ["footway", "path", "steps", "cycleway", "pedestrian", "living_street"];
const MOTOR_CLASSES: [&str; 2] = ["motorway", "trunk"];
const SECONDARY_CLASSES: [&str; 2] = ["secondary", "tertiary"];

const FLOWING_WATER_SUBTYPES: [&str; 3] = ["river", "stream", "canal"];
const TREE_COVER_LAND_SUBTYPES: [&str; 4] = ["tree", "forest", "tree_row", "wood"];

const INSTITUTIONAL_BUILDING_CLASSES: [&str; 5] = ["education", "medical_care", "civic", "government_service", "religious"];

/// Segment flags under which nothing of a way shows from above: the ground
/// is the surface, and a tunnel, an indoor passage, a line not built yet and
/// one whose track is gone are not on it. `is_disused` is not here — its
/// rails are still there — nor `is_covered`, which is under a roof that the
/// buildings paint over where they have it.
pub const UNSEEN_FLAGS: &[&str] = &["is_tunnel", "is_indoor", "is_under_construction", "is_abandoned"];

/// Segment flags under which a way runs through the ground, where a bore
/// is cut for it at a hillside (deck/).
pub const BORE_FLAGS: &[&str] = &["is_tunnel"];

/// Segment flags under which a way is a link — a slip road, one lane.
pub const LINK_FLAGS: &[&str] = &["is_link"];

pub const DECK_FLAGS: &[&str] = &["is_bridge"];

/// What every row but a way ranks at; a building over it, water under it.
const AREA_RANK: u32 = 10;
const BUILDING_RANK: u32 = 20;
const WATER_RANK: u32 = 5;
const INSTITUTIONAL_RANK: u32 = 40;

fn land_use_cell(class: &str) -> u8 {
    use cube_types::land as l;
    match class {
        "residential" => l::residential,
        "commercial" | "retail" => l::commercial,
        "construction" => l::industrial,
        "park" | "garden" | "grass" | "pedestrian" => l::green_light,
        "pitch" | "playground" => l::sports,
        "school" => l::education,
        "hospital" => l::medical,
        _ => l::bare,
    }
}

pub fn buildings(row: &BuildingRow, page_min: G3857) -> Vec<Feature> {
    use cube_types::structure as s;
    let kind = row.class.as_deref().or(row.subtype.as_deref()).unwrap_or("");
    let institutional = INSTITUTIONAL_BUILDING_CLASSES.contains(&kind);
    let value = match kind {
        "transportation" => s::building_station,
        "medical_care" => s::building_medical,
        "education" => s::building_education,
        _ if institutional => s::building_public,
        _ => s::building,
    };
    let cell = Cell { key: GroundChannelKey::STRUCTURE, value };
    let rank = BUILDING_RANK + if institutional { INSTITUTIONAL_RANK } else { 0 };
    wkb_to_geometries(&row.geom_wkb).iter().filter_map(|g| Feature::of(g, page_min, cell, rank)).collect()
}

/// A segment's parts the ground shows: not where an unseen flag holds or a
/// line draws it, cut square there, so the paint ends where the line begins;
/// none of a `water` segment, a ferry route or a strait's label line, nor of a
/// crossing, a line over a road the road already shows.
pub fn ways(row: &Segment, drawn: &[Drawn], page_min: G3857) -> Vec<Feature> {
    if row.subtype == "water" {
        return Vec::new();
    }
    let unseen = flag_ranges(row.flags.as_deref(), UNSEEN_FLAGS, cell_of(length_of(&row.geom_wkb)));
    let lined = merged(drawn.iter().filter(|c| c.gers_id == row.gers_id).map(|c| c.range).collect(), 0.0);
    let cell = Cell {
        key: GroundChannelKey::TRANSPORT,
        value: segment_type(&row.subtype, row.class.as_deref()),
    };
    let cell_per_m = u_per_m(from_db(page_min[1])) / U_PER_CELL;
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .flat_map(|g| split_ranged(g, [&unseen, &lined]))
        .filter(|([unseen, lined], ..)| !unseen && !lined)
        .filter_map(|(_, [a, b], g)| {
            let shape = WayShape::at(row.columns(), (a + b) / 2.0);
            if shape.role == Role::Crossing {
                return None;
            }
            let mut f = Feature::of(&g, page_min, cell, paint_order(&row.subtype, row.class.as_deref(), shape.role))?;
            f.closed = false;
            f.width = (width_m(&row.subtype, row.class.as_deref(), shape) * cell_per_m).min(WIDEST);
            f.cut = [a > 0.0, b < 1.0];
            f.joins = match shape.role {
                Role::Sidewalk => Some(Joins::Road),
                _ if shape.oneway && row.subtype == "road" && THROUGH_CLASSES.contains(&row.class.as_deref().unwrap_or("")) => Some(Joins::Carriageway),
                _ => None,
            };
            Some(f)
        })
        .collect()
}

/// Water but a strait's label line, land under trees, parking and helipads,
/// and every land use.
pub fn bases(row: &BaseRow, page_min: G3857) -> Vec<Feature> {
    use cube_types::{land as l, transport as t, water as w};
    let (subtype, class) = (row.subtype.as_deref().unwrap_or(""), row.class.as_deref().unwrap_or(""));
    let (key, value, rank) = match row.source.as_str() {
        "water" if class == "strait" => return Vec::new(),
        "water" => (GroundChannelKey::WATER, if FLOWING_WATER_SUBTYPES.contains(&subtype) { w::river } else { w::lake }, WATER_RANK),
        "land" if TREE_COVER_LAND_SUBTYPES.contains(&subtype) => (GroundChannelKey::LAND, l::green_deep, AREA_RANK),
        "land" => return Vec::new(),
        "infrastructure" if class == "parking" => (GroundChannelKey::TRANSPORT, t::parking, AREA_RANK),
        "infrastructure" if class == "helipad" => (GroundChannelKey::LAND, l::helipad, AREA_RANK),
        "infrastructure" => return Vec::new(),
        _ => (GroundChannelKey::LAND, land_use_cell(class), AREA_RANK + if matches!(class, "school" | "hospital") { INSTITUTIONAL_RANK } else { 0 }),
    };
    wkb_to_geometries(&row.geom_wkb).iter().filter_map(|g| Feature::of(g, page_min, Cell { key, value }, rank)).collect()
}

/// Later keeps the cell. OpenStreetMap Carto's `z` from `roads_info`
/// (openstreetmap-carto-flex.lua, CC0); other transport features rank under all.
fn paint_order(subtype: &str, class: Option<&str>, role: Role) -> u32 {
    let c = class.unwrap_or("");
    if subtype == "rail" {
        return match c {
            "standard_gauge" => 440,
            "tram" => 410,
            _ => 420,
        };
    }
    if role == Role::Link {
        return match c {
            "motorway" => 240,
            "trunk" => 230,
            "primary" => 220,
            "secondary" => 210,
            _ => 200,
        };
    }
    if role == Role::Sidewalk {
        return 100;
    }
    if role == Role::Lane {
        return 150;
    }
    match c {
        "motorway" => 380,
        "trunk" => 370,
        "primary" => 360,
        "secondary" => 350,
        "tertiary" => 340,
        "living_street" => 320,
        "pedestrian" => 310,
        "service" => 150,
        "track" => 110,
        "path" | "footway" | "bridleway" | "cycleway" => 100,
        "steps" => 90,
        _ => 330,
    }
}
