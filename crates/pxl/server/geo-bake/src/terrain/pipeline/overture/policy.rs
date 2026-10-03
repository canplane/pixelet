// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings/Transportation/Base rows to categories, metrics and cell
//! config. Segment and base values come from a Gangnam sample; building
//! classes from the published schema (only ~14% of Korean buildings have one).

use pxl_spec::bake::format::{CellConfig, GroundChannelKey};
use pxl_spec::bake::model::feature::{FeatureComputed, FeatureExtra, Joins};
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::normalizer::{NormalizePolicy, RawFeature};
use crate::terrain::pipeline::overture::adapter::{BaseSource, OvertureRawFeature};
use crate::terrain::pipeline::overture::role::Role;
use crate::terrain::pipeline::overture::width::width_m;

/// One type per OpenMapTiles transportation class (MapTiler's schema).
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

/// One-way ways of these classes are carriageways, not one-way streets.
const THROUGH_CLASSES: [&str; 5] = ["motorway", "trunk", "primary", "secondary", "tertiary"];

/// OpenMapTiles' path class.
const PEDESTRIAN_CLASSES: [&str; 6] = ["footway", "path", "steps", "cycleway", "pedestrian", "bridleway"];

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

pub const BORE_FLAGS: &[&str] = &["is_tunnel"];

pub const LINK_FLAGS: &[&str] = &["is_link"];

/// Drawn by the page's decks, not the ground.
pub const DECK_FLAGS: &[&str] = &["is_bridge"];

fn building_type(feat: &OvertureRawFeature) -> &str {
    match feat {
        OvertureRawFeature::Building(b) => b.class.as_deref().or(b.subtype.as_deref()).unwrap_or(""),
        _ => "",
    }
}

fn land_use_class(feat: &OvertureRawFeature) -> &str {
    match feat {
        OvertureRawFeature::Base(b) if b.source == BaseSource::LandUse => b.class.as_deref().unwrap_or(""),
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
            height = match (b.height, b.num_floors) {
                (Some(h), _) => h,
                (None, Some(n)) => n * 3.5,
                (None, None) => 3.5,
            };
        }
        OvertureRawFeature::Segment(s) => {
            width = width_m(&s.subtype, s.class.as_deref(), s.shape);
        }
        OvertureRawFeature::Base(_) => {}
    }

    FeatureComputed {
        height: height.max(0.0),
        width: width.max(0.0),
        // `level_rules` is read, but nothing is placed off the surface yet.
        layer: 0,
    }
}

/// A `water` segment is a ferry route or a strait's label line, not on the
/// surface, and no mask would stop it painting over the sea.
fn is_unseen(feat: &OvertureRawFeature) -> bool {
    match feat {
        OvertureRawFeature::Segment(s) => s.is_unseen || s.subtype == "water",
        OvertureRawFeature::Base(b) => b.source == BaseSource::Water && b.class.as_deref() == Some("strait"),
        OvertureRawFeature::Building(_) => false,
    }
}

fn is_institutional(feat: &OvertureRawFeature) -> bool {
    let land_use = land_use_class(feat);
    INSTITUTIONAL_BUILDING_CLASSES.contains(&building_type(feat)) || land_use == "school" || land_use == "hospital"
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

    if let OvertureRawFeature::Segment(s) = feat {
        return paint_order(&s.subtype, s.class.as_deref(), s.shape.role);
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
        cut: match feat {
            OvertureRawFeature::Segment(s) => s.cut,
            _ => [false; 2],
        },
        joins: match feat {
            OvertureRawFeature::Segment(s) if s.shape.role == Role::Sidewalk => Some(Joins::Road),
            OvertureRawFeature::Segment(s) if s.shape.oneway && s.subtype == "road" && THROUGH_CLASSES.contains(&s.class.as_deref().unwrap_or("")) => Some(Joins::Carriageway),
            _ => None,
        },
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
        // A crossing is a line over a road the road already shows.
        if s.shape.role == Role::Crossing {
            return None;
        }
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
