// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings/Transportation/Base rows to categories, metrics and cell
//! config. Segment and base values come from a Gangnam sample; building
//! classes from the published schema (only ~14% of Korean buildings have one).

use pxl_spec::bake::format::{CellConfig, GroundChannelKey};
use pxl_spec::bake::model::feature::{FeatureComputed, FeatureExtra};
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::normalizer::{NormalizePolicy, RawFeature};
use crate::terrain::pipeline::overture::adapter::{BaseSource, OvertureRawFeature};

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

fn land_use_class_map(class: &str) -> Option<u8> {
    Some(match class {
        "residential" => cube_types::land::residential,
        "commercial" | "retail" => cube_types::land::commercial,
        "construction" => cube_types::land::industrial,
        "park" | "garden" | "grass" | "pedestrian" => cube_types::land::green_light,
        "pitch" | "playground" => cube_types::land::sports,
        "school" => cube_types::land::education,
        "hospital" => cube_types::land::medical,
        _ => return None,
    })
}

const INSTITUTIONAL_BUILDING_CLASSES: [&str; 5] = ["education", "medical_care", "civic", "government_service", "religious"];
const SPORTS_CLASSES: [&str; 2] = ["pitch", "playground"];

/// Nothing of a way under these shows from above. Not `is_disused` (the rails
/// remain) nor `is_covered` (the buildings paint over it).
pub const UNSEEN_FLAGS: &[&str] = &["is_tunnel", "is_indoor", "is_under_construction", "is_abandoned"];

/// Segment flags under which a way is carried above the ground: a deck,
/// which the page's decks draw (terrain/pipeline/deck.rs) and the ground
/// does not.
pub const DECK_FLAGS: &[&str] = &["is_bridge"];

// ============ Policy logic ============

fn building_type(feat: &OvertureRawFeature) -> &str {
    match feat {
        OvertureRawFeature::Building(b) => b.class.as_deref().or(b.subtype.as_deref()).unwrap_or(""),
        _ => "",
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
    let cell = Cell {
        key: GroundChannelKey::STRUCTURE,
        value,
    };
    let rank = BUILDING_RANK + if institutional { INSTITUTIONAL_RANK } else { 0 };
    wkb_to_geometries(&row.geom_wkb).iter().filter_map(|g| Feature::of(g, page_min, cell, rank)).collect()
}

fn segment_class(feat: &OvertureRawFeature) -> &str {
    match feat {
        OvertureRawFeature::Segment(s) => s.class.as_deref().unwrap_or(""),
        _ => "",
    }
}

fn determine_category(feat: &OvertureRawFeature) -> String {
    match feat {
        OvertureRawFeature::Building(_) => "building".to_string(),
        OvertureRawFeature::Segment(_) => "road".to_string(),
        OvertureRawFeature::Base(b) => match b.source {
            BaseSource::Water => "water".to_string(),
            BaseSource::Land => {
                if TREE_COVER_LAND_SUBTYPES.contains(&b.subtype.as_deref().unwrap_or("")) {
                    "forest".to_string()
                } else {
                    "landuse".to_string()
                }
            }
            BaseSource::Infrastructure => {
                if b.class.as_deref() == Some("parking") {
                    "parking".to_string()
                } else {
                    "landuse".to_string()
                }
            }
            BaseSource::LandUse => "landuse".to_string(),
        },
    }
}

fn compute_metrics(feat: &OvertureRawFeature, _category: &str) -> FeatureComputed {
    let mut height = 0.0;
    let mut width = 0.0;

    match feat {
        OvertureRawFeature::Building(b) => {
            // Real height/num_floors when Overture has them; missing both
            // falls back to a single-level default.
            height = match (b.height, b.num_floors) {
                (Some(h), _) => h,
                (None, Some(n)) => n * 3.5,
                (None, None) => 3.5,
            };
        }
        OvertureRawFeature::Segment(_) => {
            width = segment_width_m(segment_class(feat));
        }
        OvertureRawFeature::Base(_) => {}
    }

    FeatureComputed {
        height: height.max(0.0),
        width: width.max(0.0),
        // `level_rules` is in the table, but nothing places a feature off the
        // surface yet: what is seen from above is painted on the ground.
        layer: 0,
    }
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

fn calculate_rank(feat: &OvertureRawFeature, category: &str, is_institutional: bool) -> u32 {
    let mut score: u32 = 10;

    if category == "water" {
        score = 5;
    }
    if category == "building" {
        score += 10;
    }

    if matches!(feat, OvertureRawFeature::Segment(_)) {
        let c = segment_class(feat);
        if MOTOR_CLASSES.contains(&c) {
            score += 80;
        } else if c == "primary" || SECONDARY_CLASSES.contains(&c) {
            score += 50;
        } else {
            score += 10;
        }
    }

    if is_institutional {
        score += 40;
    }

    score
}

fn compute_extra(feat: &OvertureRawFeature, category: &str) -> FeatureExtra {
    let institutional = is_institutional(feat);
    FeatureExtra {
        rank: calculate_rank(feat, category, institutional),
        is_ocean: false,
        is_parking: category == "parking",
        is_institutional: institutional,
        is_sports: SPORTS_CLASSES.contains(&building_type(feat)) || SPORTS_CLASSES.contains(&land_use_class(feat)),
        is_unseen: is_unseen(feat),
        is_deck: matches!(feat, OvertureRawFeature::Segment(s) if s.is_deck),
    }
}

fn resolve_land(feat: &OvertureRawFeature, category: &str, extra: &FeatureExtra) -> Option<u8> {
    if category == "forest" {
        return Some(cube_types::land::green_deep);
    }

    if let OvertureRawFeature::Base(b) = feat {
        if b.source == BaseSource::Infrastructure && b.class.as_deref() == Some("helipad") {
            return Some(cube_types::land::helipad);
        }
        if b.source == BaseSource::LandUse {
            if let Some(mapped) = land_use_class_map(b.class.as_deref().unwrap_or("")) {
                return Some(mapped);
            }
            if extra.is_institutional {
                return Some(cube_types::land::public);
            }
            if extra.is_sports {
                return Some(cube_types::land::sports);
            }
            return Some(cube_types::land::bare);
        }
    }

    None
}

fn determine_cell_cfg(feat: &OvertureRawFeature, category: &str, extra: &FeatureExtra) -> Option<CellConfig> {
    if let OvertureRawFeature::Base(b) = feat {
        if category == "water" {
            let cell_type = if FLOWING_WATER_SUBTYPES.contains(&b.subtype.as_deref().unwrap_or("")) {
                cube_types::water::river
            } else {
                cube_types::water::lake
            };
            return Some(CellConfig {
                cell_type,
                ch_key: GroundChannelKey::WATER,
            });
        }
    }

    if matches!(feat, OvertureRawFeature::Building(_)) && category == "building" {
        let cell_type = match building_type(feat) {
            "transportation" => cube_types::structure::building_station,
            "medical_care" => cube_types::structure::building_medical,
            "education" => cube_types::structure::building_education,
            _ if extra.is_institutional => cube_types::structure::building_public,
            _ => cube_types::structure::building,
        };
        return Some(CellConfig {
            cell_type,
            ch_key: GroundChannelKey::STRUCTURE,
        });
    }

    if let OvertureRawFeature::Segment(s) = feat {
        if category == "road" {
            return Some(CellConfig {
                cell_type: segment_type(&s.subtype, s.class.as_deref()),
                ch_key: GroundChannelKey::TRANSPORT,
            });
        }
    }

    if category == "parking" {
        return Some(CellConfig {
            cell_type: cube_types::transport::parking,
            ch_key: GroundChannelKey::TRANSPORT,
        });
    }

    resolve_land(feat, category, extra).map(|cell_type| CellConfig {
        cell_type,
        ch_key: GroundChannelKey::LAND,
    })
}

pub struct OverturePolicy;

impl NormalizePolicy<OvertureRawFeature> for OverturePolicy {
    fn get_id(&self, raw: &OvertureRawFeature) -> String {
        match raw {
            OvertureRawFeature::Building(b) => b.gers_id.clone(),
            OvertureRawFeature::Segment(s) => s.gers_id.clone(),
            OvertureRawFeature::Base(b) => b.gers_id.clone(),
        }
    }

    fn determine_category(&self, raw: &OvertureRawFeature) -> String {
        determine_category(raw)
    }

    fn compute_metrics(&self, raw: &OvertureRawFeature, category: &str) -> FeatureComputed {
        compute_metrics(raw, category)
    }

    fn compute_extra(&self, raw: &OvertureRawFeature, category: &str) -> FeatureExtra {
        compute_extra(raw, category)
    }

    fn determine_cell_cfg(&self, raw: &OvertureRawFeature, category: &str, extra: &FeatureExtra) -> Option<CellConfig> {
        determine_cell_cfg(raw, category, extra)
    }

    fn should_force_closed(&self, _raw: &OvertureRawFeature) -> bool {
        false
    }

    fn is_linear(&self, raw: &OvertureRawFeature) -> bool {
        matches!(raw, OvertureRawFeature::Segment(_))
    }
}

impl RawFeature for OvertureRawFeature {
    fn points(&self) -> &[G3857] {
        match self {
            OvertureRawFeature::Building(b) => &b.geometry.points,
            OvertureRawFeature::Segment(s) => &s.geometry.points,
            OvertureRawFeature::Base(b) => &b.geometry.points,
        }
    }

    fn holes(&self) -> &[Vec<G3857>] {
        match self {
            OvertureRawFeature::Building(b) => &b.geometry.holes,
            OvertureRawFeature::Segment(s) => &s.geometry.holes,
            OvertureRawFeature::Base(b) => &b.geometry.holes,
        }
    }

    fn is_polygon_source(&self) -> bool {
        match self {
            OvertureRawFeature::Building(b) => b.geometry.is_polygon_source,
            OvertureRawFeature::Segment(s) => s.geometry.is_polygon_source,
            OvertureRawFeature::Base(b) => b.geometry.is_polygon_source,
        }
    }

    fn center(&self) -> Option<G3857> {
        Some(match self {
            OvertureRawFeature::Building(b) => b.center,
            OvertureRawFeature::Segment(s) => s.center,
            OvertureRawFeature::Base(b) => b.center,
        })
    }
}
