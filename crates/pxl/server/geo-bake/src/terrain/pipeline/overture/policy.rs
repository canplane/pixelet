// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings, Transportation and Base rows to what the ground paints.
//! Segment and base values come from a Gangnam sample; building classes from
//! the published schema (only ~14% of Korean buildings have one).

use pxl_spec::bake::format::{CellConfig, GroundChannelKey};
use pxl_spec::bake::model::feature::{FeatureComputed, FeatureExtra, Joins};
use pxl_spec::cell::cube::cube_types;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m, G3857};

use crate::terrain::pipeline::normalizer::{NormalizePolicy, RawFeature};
use crate::terrain::pipeline::overture::adapter::{BaseSource, OvertureRawFeature};
use crate::terrain::pipeline::overture::role::Role;
use crate::terrain::pipeline::overture::width::width_m;

// ============ Domain knowledge constants ============

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

/// The classes whose one-way ways are a road's carriageways rather than a
/// one-way street of their own.
const THROUGH_CLASSES: [&str; 5] = ["motorway", "trunk", "primary", "secondary", "tertiary"];

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

/// Segment flags under which a way is a link — a slip road, one lane.
pub const LINK_FLAGS: &[&str] = &["is_link"];

/// Segment flags under which a way is carried above the ground: a deck,
/// which the page's decks draw (terrain/pipeline/deck.rs) and the ground
/// does not.
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
            let mut f = Feature::of(&g, page_min, cell, paint_order(&row.subtype, row.class.as_deref(), shape.role))?;
            f.closed = false;
            f.width = (width_m(&row.subtype, row.class.as_deref(), shape) * cell_per_m).min(WIDEST);
            f.cut = [a > 0.0, b < 1.0];
            f.joins = match shape.role {
                Role::Sidewalk => Some(Joins::Road),
                _ if shape.oneway && row.subtype == "road" && THROUGH_CLASSES.contains(&row.class.as_deref().unwrap_or("")) => Some(Joins::Carriageway),
                _ => None,
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
        // `level_rules` is in the table, but nothing places a feature off the
        // surface yet: what is seen from above is painted on the ground.
        layer: 0,
    }
}

/// Whether nothing of a feature shows from above. A `water` segment is a
/// ferry's route and a strait's line is where its name is written: neither is
/// a thing on the surface, and a route paints over the sea, which no mask
/// stops a transport cell at.
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

/// Where a way paints among the transport channel's, lowest first: where
/// two overlap — a junction, a road beside its sidewalk, a railway across a
/// street — the later keeps the cell. Every other feature on the channel, a
/// parking lot, ranks under all of them (`calculate_rank`'s base of ten).
///
/// The order is OpenStreetMap Carto's, the standard map's (`roads_info` in
/// `openstreetmap-carto-flex.lua`, CC0), its `z` taken as is: a footway or a
/// sidewalk under a service lane under every link, the links under every
/// through road in their classes' order, the through roads in theirs, and a
/// railway over them all, as a level crossing is drawn.
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
        // Sea fill is coast/policy.rs's job, never this policy's.
        is_ocean: false,
        is_parking: category == "parking",
        is_institutional: institutional,
        is_sports: SPORTS_CLASSES.contains(&building_type(feat)) || SPORTS_CLASSES.contains(&land_use_class(feat)),
        is_unseen: is_unseen(feat),
        is_deck: matches!(feat, OvertureRawFeature::Segment(s) if s.is_deck),
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
