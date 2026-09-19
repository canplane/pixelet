// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Policy for interpreting Overture Buildings/Transportation/Base rows into
//! categories, metrics, flags and terrain cell configuration — the
//! counterpart to coast/policy.rs (coastline only).
//!
//! Mapped value by value against each theme's own subtype/class vocabulary.
//! The Segments/Land_use/Water/Land class and subtype values below are the
//! ones a real Gangnam-area sample actually produced (100% populated for all
//! four); Buildings' class enum is not sample-confirmed the same way (only
//! ~14% of Korean buildings carry `class`/`subtype` at all) — mapped against
//! Overture's published buildings schema instead, and it barely matters
//! either way since only the coarse structure category is ever baked.

use pxl_spec::bake::format::{CellConfig, GroundChannelKey};
use pxl_spec::bake::model::feature::{FeatureComputed, FeatureExtra};
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::adapter::epsg3857::G3857;

use crate::terrain::pipeline::normalizer::{NormalizePolicy, RawFeature};
use crate::terrain::pipeline::overture::adapter::{BaseSource, OvertureRawFeature};

// ============ Domain knowledge constants ============

/// Overture Transportation `class` -> default width in meters, absent
/// structured `road` lane data.
fn road_width_by_class(class: &str) -> Option<f64> {
    Some(match class {
        "motorway" => 14.0,
        "trunk" | "primary" => 10.5,
        "secondary" | "tertiary" => 7.0,
        "residential" | "service" | "unclassified" | "living_street" => 3.5,
        "path" | "footway" | "steps" | "cycleway" | "pedestrian" => 1.75,
        "standard_gauge" | "light_rail" | "subway" => 3.5,
        _ => return None,
    })
}

const PEDESTRIAN_CLASSES: [&str; 6] = ["footway", "path", "steps", "cycleway", "pedestrian", "living_street"];
const MOTOR_CLASSES: [&str; 2] = ["motorway", "trunk"];
const SECONDARY_CLASSES: [&str; 2] = ["secondary", "tertiary"];

const FLOWING_WATER_SUBTYPES: [&str; 3] = ["river", "stream", "canal"];
const TREE_COVER_LAND_SUBTYPES: [&str; 4] = ["tree", "forest", "tree_row", "wood"];

/// Land_use `class` -> land cell, using the values the sample turned up.
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

/// Buildings `class`/`subtype` treated as institutional (against Overture's
/// published schema, not sample-confirmed — see this file's top comment).
const INSTITUTIONAL_BUILDING_CLASSES: [&str; 5] = ["education", "medical_care", "civic", "government_service", "religious"];
const SPORTS_CLASSES: [&str; 2] = ["pitch", "playground"];

// ============ Policy logic ============

/// `feat.class ?? feat.subtype ?? ""`, the building's own type as the policy
/// reads it.
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
            width = road_width_by_class(segment_class(feat)).unwrap_or(3.5);
        }
        OvertureRawFeature::Base(_) => {}
    }

    FeatureComputed {
        height: height.max(0.0),
        width: width.max(0.0),
        // No bridge/tunnel vertical-stacking signal is ingested for Overture yet.
        layer: 0,
    }
}

fn is_institutional(feat: &OvertureRawFeature) -> bool {
    let land_use = land_use_class(feat);
    INSTITUTIONAL_BUILDING_CLASSES.contains(&building_type(feat)) || land_use == "school" || land_use == "hospital"
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
        // Sea fill is coast/policy.rs's job, never this policy's.
        is_ocean: false,
        is_parking: category == "parking",
        is_institutional: institutional,
        is_sports: SPORTS_CLASSES.contains(&building_type(feat)) || SPORTS_CLASSES.contains(&land_use_class(feat)),
        // Overture's segment `road` JSONB likely carries a tunnel/bridge flag,
        // but its real shape is not sample-confirmed, so this stays false
        // until a live sample settles the field path — see the top comment on
        // not guessing against unverified schema.
        is_tunnel: false,
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
            let c = s.class.as_deref().unwrap_or("");
            let cell_type = if s.subtype == "rail" {
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
            };
            return Some(CellConfig {
                cell_type,
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

    /// Every theme but segments is WKB polygon geometry already
    /// (`is_polygon_source` covers closure), so there is nothing left to force.
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
