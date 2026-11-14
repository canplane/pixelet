// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings, Transportation and Base rows to the ground cube each paints, as
//! OpenStreetMap Carto paints them (gravitystorm/openstreetmap-carto 1cc4b89). An
//! Overture class is the value of the OSM tag it comes from, so it takes the cube, the
//! layer and the `feature` Carto gives that tag (`landcover.mss`,
//! `water.mss`, `buildings.mss`, `roads.mss`, `project.mml`); a class Carto
//! draws no fill for paints nothing, and the land under it shows.

use pxl_spec::asset::cube::Ground;
use pxl_spec::ground::GroundId;
use pxl_spec::space::coord::adapter::epsg3857::G3857;

use crate::segment::Part;
use crate::separation::WayRange;
use crate::terrain::pipeline::feature::{closes, Feature, Layer, Rank};
use crate::terrain::pipeline::overture::between::{merged, uncovered, Line};
use crate::terrain::pipeline::overture::role::Role;
use crate::terrain::pipeline::overture::width::{carto_u, waterway_width, width_u, WIDEST_LINE};
use crate::terrain::repository::{BaseRow, BuildingRow};
use crate::wkb::wkb_to_geometries;

/// A way's ground asset id: Carto's `highway` and `railway` values, its white roads one
/// asset id told apart by width, `path` and `steps` footways.
pub(crate) fn segment_type(subtype: &str, class: Option<&str>) -> GroundId {
    let c = class.unwrap_or("");
    if subtype == "rail" {
        return match c {
            "tram" => Ground::Tram as GroundId,
            "light_rail" | "narrow_gauge" | "funicular" => Ground::LightRail as GroundId,
            "subway" => Ground::Subway as GroundId,
            "monorail" => Ground::Monorail as GroundId,
            _ => Ground::Rail as GroundId,
        };
    }
    match c {
        "motorway" => Ground::Motorway as GroundId,
        "trunk" => Ground::Trunk as GroundId,
        "primary" => Ground::Primary as GroundId,
        "secondary" => Ground::Secondary as GroundId,
        "tertiary" | "residential" | "unclassified" | "service" => Ground::ResidentialRoad as GroundId,
        "living_street" => Ground::LivingStreet as GroundId,
        "pedestrian" => Ground::Pedestrian as GroundId,
        "track" => Ground::Track as GroundId,
        "cycleway" => Ground::Cycleway as GroundId,
        "bridleway" => Ground::Bridleway as GroundId,
        "footway" | "path" | "steps" => Ground::Footway as GroundId,
        _ => Ground::Road as GroundId,
    }
}

/// Carto's `building-major-fill`: a place of worship, a terminal or a station.
/// Overture's `religious` subtype with no class is `amenity=place_of_worship`.
const MAJOR_BUILDING_CLASSES: [&str; 9] = [
    "cathedral",
    "chapel",
    "church",
    "mosque",
    "religious",
    "shrine",
    "synagogue",
    "temple",
    "train_station",
];

/// What a Base row paints: its ground asset id, Carto's layer, and Carto's `feature`.
type Paint = (GroundId, Layer, &'static str);

/// `landuse`, `leisure`, `amenity` and the like, as Overture's land use
/// classes carry them.
fn land_use_paint(class: &str) -> Option<Paint> {
    use Layer::{Landcover as L, RoadArea};
    Some(match class {
        "residential" => (Ground::Residential as GroundId, L, "landuse_residential"),
        "garages" => (Ground::Garages as GroundId, L, "landuse_garages"),
        "retail" => (Ground::Retail as GroundId, L, "landuse_retail"),
        "commercial" => (Ground::Commercial as GroundId, L, "landuse_commercial"),
        "industrial" => (Ground::Industrial as GroundId, L, "landuse_industrial"),
        "works" => (Ground::Industrial as GroundId, L, "man_made_works"),
        "landfill" => (Ground::Landfill as GroundId, L, "landuse_landfill"),
        "quarry" => (Ground::Quarry as GroundId, L, "landuse_quarry"),
        "farmland" => (Ground::Farmland as GroundId, L, "landuse_farmland"),
        "greenhouse_horticulture" => (Ground::Farmland as GroundId, L, "landuse_greenhouse_horticulture"),
        "farmyard" => (Ground::Farmyard as GroundId, L, "landuse_farmyard"),
        "orchard" => (Ground::Orchard as GroundId, L, "landuse_orchard"),
        "vineyard" => (Ground::Orchard as GroundId, L, "landuse_vineyard"),
        "plant_nursery" => (Ground::Orchard as GroundId, L, "landuse_plant_nursery"),
        "allotments" => (Ground::Allotments as GroundId, L, "landuse_allotments"),
        "cemetery" => (Ground::Cemetery as GroundId, L, "landuse_cemetery"),
        "grave_yard" => (Ground::Cemetery as GroundId, L, "amenity_grave_yard"),
        "meadow" => (Ground::Grass as GroundId, L, "landuse_meadow"),
        "grass" => (Ground::Grass as GroundId, L, "landuse_grass"),
        "village_green" => (Ground::Grass as GroundId, L, "landuse_village_green"),
        "flowerbed" => (Ground::Grass as GroundId, L, "landuse_flowerbed"),
        "garden" => (Ground::Grass as GroundId, L, "leisure_garden"),
        "park" => (Ground::Park as GroundId, L, "leisure_park"),
        "recreation_ground" => (Ground::Leisure as GroundId, L, "landuse_recreation_ground"),
        "playground" => (Ground::Leisure as GroundId, L, "leisure_playground"),
        "dog_park" => (Ground::Leisure as GroundId, L, "leisure_dog_park"),
        "stadium" => (Ground::Leisure as GroundId, L, "leisure_stadium"),
        "water_park" => (Ground::Leisure as GroundId, L, "leisure_water_park"),
        "pitch" => (Ground::Pitch as GroundId, L, "leisure_pitch"),
        "track" => (Ground::Pitch as GroundId, L, "leisure_track"),
        "golf_course" => (Ground::Campsite as GroundId, L, "leisure_golf_course"),
        "camp_site" => (Ground::Campsite as GroundId, L, "tourism_camp_site"),
        "religious" => (Ground::PlaceOfWorship as GroundId, L, "landuse_religious"),
        "school" => (Ground::SocietalAmenities as GroundId, L, "amenity_school"),
        "college" => (Ground::SocietalAmenities as GroundId, L, "amenity_college"),
        "university" => (Ground::SocietalAmenities as GroundId, L, "amenity_university"),
        "kindergarten" => (Ground::SocietalAmenities as GroundId, L, "amenity_kindergarten"),
        "hospital" => (Ground::SocietalAmenities as GroundId, L, "amenity_hospital"),
        "clinic" => (Ground::SocietalAmenities as GroundId, L, "amenity_clinic"),
        "salt_pond" => (Ground::Water as GroundId, L, "landuse_salt_pond"),
        "pedestrian" | "plaza" => (Ground::Pedestrian as GroundId, RoadArea, "highway_pedestrian"),
        _ => return None,
    })
}

/// `golf.mss`, by the `golf` tag; a water hazard has no fill.
fn golf_paint(class: &str) -> Option<Paint> {
    use Layer::Landcover as L;
    Some(match class {
        "tee" => (Ground::Grass as GroundId, L, "golf_tee"),
        "fairway" => (Ground::Grass as GroundId, L, "golf_fairway"),
        "rough" => (Ground::Grass as GroundId, L, "golf_rough"),
        "driving_range" => (Ground::Grass as GroundId, L, "golf_driving_range"),
        "green" => (Ground::Pitch as GroundId, L, "golf_green"),
        "bunker" => (Ground::Sand as GroundId, L, "golf_bunker"),
        _ => return None,
    })
}

/// `natural`, as Overture's land classes carry it; Carto fills a wetland by
/// its `wetland` tag, which Overture drops, and one without it not at all.
/// The only land classes read: the rest paint nothing.
pub(crate) const LAND: &[(&str, Paint)] = {
    use Layer::Landcover as L;
    &[
        ("wood", (Ground::Forest as GroundId, L, "natural_wood")),
        ("forest", (Ground::Forest as GroundId, L, "landuse_forest")),
        ("bare_rock", (Ground::BareGround as GroundId, L, "natural_bare_rock")),
        ("scree", (Ground::BareGround as GroundId, L, "natural_scree")),
        ("shingle", (Ground::BareGround as GroundId, L, "natural_shingle")),
        ("sand", (Ground::Sand as GroundId, L, "natural_sand")),
        ("beach", (Ground::Beach as GroundId, L, "natural_beach")),
        ("heath", (Ground::Heath as GroundId, L, "natural_heath")),
        ("scrub", (Ground::Scrub as GroundId, L, "natural_scrub")),
        ("grassland", (Ground::Grass as GroundId, L, "natural_grassland")),
        ("glacier", (Ground::Glacier as GroundId, Layer::Water, "natural_glacier")),
    ]
};

fn land_paint(class: &str) -> Option<Paint> {
    LAND.iter()
        .find(|(c, _)| *c == class)
        .map(|&(_, paint)| paint)
}

/// Water areas but what Carto draws as landcover in water's color or as a
/// shoal; a physical feature or a spring (a bay, a strait) has no fill.
fn water_paint(subtype: &str, class: &str, closed: bool) -> Option<Paint> {
    use Layer::Landcover as L;
    if !closed {
        return waterway_width(class).map(|_| (Ground::Water as GroundId, Layer::WaterLine, ""));
    }
    Some(match (subtype, class) {
        (_, "shoal") => (Ground::Beach as GroundId, L, "natural_shoal"),
        (_, "salt_pond") => (Ground::Water as GroundId, L, "landuse_salt_pond"),
        (_, "swimming_pool") => (Ground::Water as GroundId, L, "leisure_swimming_pool"),
        ("physical" | "spring", _) => return None,
        _ => (Ground::Water as GroundId, Layer::Water, ""),
    })
}

/// The infrastructure classes with a Carto fill, the only ones read. An
/// aeroway's area is `highway-area-fill`'s, its line the `aeroways` layer's.
pub(crate) const INFRASTRUCTURE: &[(&str, Paint)] = {
    use Layer::{Landcover as L, RoadArea};
    &[
        ("parking", (Ground::Parking as GroundId, L, "amenity_parking")),
        (
            "bicycle_parking",
            (Ground::Parking as GroundId, L, "amenity_bicycle_parking"),
        ),
        (
            "motorcycle_parking",
            (Ground::Parking as GroundId, L, "amenity_motorcycle_parking"),
        ),
        ("apron", (Ground::Apron as GroundId, L, "aeroway_apron")),
        ("airport", (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome")),
        (
            "international_airport",
            (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome"),
        ),
        (
            "municipal_airport",
            (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome"),
        ),
        (
            "regional_airport",
            (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome"),
        ),
        (
            "private_airport",
            (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome"),
        ),
        (
            "military_airport",
            (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome"),
        ),
        (
            "bus_station",
            (Ground::TransportationArea as GroundId, L, "amenity_bus_station"),
        ),
        (
            "ferry_terminal",
            (Ground::TransportationArea as GroundId, L, "amenity_ferry_terminal"),
        ),
        ("railway_station", (Ground::Industrial as GroundId, L, "railway_station")),
        ("subway_station", (Ground::Industrial as GroundId, L, "railway_station")),
        ("substation", (Ground::Power as GroundId, L, "power_substation")),
        ("generator", (Ground::Power as GroundId, L, "power_generator")),
        ("plant", (Ground::Power as GroundId, L, "power_plant")),
        ("platform", (Ground::Platform as GroundId, RoadArea, "railway_platform")),
        ("runway", (Ground::Aeroway as GroundId, RoadArea, "aeroway_runway")),
        ("taxiway", (Ground::Aeroway as GroundId, RoadArea, "aeroway_taxiway")),
        ("helipad", (Ground::Aeroway as GroundId, RoadArea, "aeroway_helipad")),
        (
            "seaplane_airport",
            (Ground::TransportationArea as GroundId, L, "aeroway_aerodrome"),
        ),
        ("dam", (Ground::Dam as GroundId, Layer::WaterBarrier, "waterway_dam")),
        (
            "weir",
            (Ground::Breakwater as GroundId, Layer::WaterBarrier, "waterway_weir"),
        ),
        (
            "breakwater",
            (Ground::Breakwater as GroundId, Layer::Pier, "man_made_breakwater"),
        ),
        ("pier", (Ground::Land as GroundId, Layer::Pier, "man_made_pier")),
        (
            "cutline",
            (Ground::Grass as GroundId, Layer::LandcoverLine, "man_made_cutline"),
        ),
    ]
};

/// Carto draws these as lines alone.
const LINE_ONLY: [&str; 2] = ["weir", "cutline"];

fn infrastructure_paint(class: &str, closed: bool) -> Option<Paint> {
    let &(_, (asset_id, layer, feat)) = INFRASTRUCTURE.iter().find(|(c, _)| *c == class)?;
    if closed && LINE_ONLY.contains(&class) {
        return None;
    }
    Some((
        asset_id,
        if asset_id == Ground::Aeroway as GroundId && !closed {
            Layer::Aeroway
        } else {
            layer
        },
        feat,
    ))
}

/// A Base line's width in u: Carto's at z16, as a way's
/// (`overture/width.rs`), its casing included.
fn line_width(src: &str, class: &str) -> f64 {
    if src == "water" {
        return waterway_width(class).unwrap_or(0.0);
    }
    carto_u(match class {
        "runway" => WIDEST_LINE,
        "taxiway" => 8.0,
        "dam" | "breakwater" => 4.0,
        "pier" | "weir" => 2.0,
        "platform" | "cutline" => 6.0,
        _ => 0.0,
    })
}

/// Carto's `water-barriers` and `piers` stack their area layer under their
/// line layer, and among piers a pier over a breakwater.
fn barrier_order(class: &str, closed: bool) -> i64 {
    4 * i64::from(!closed) + if class == "pier" { 3 } else { 1 }
}

pub(crate) fn buildings(row: &BuildingRow, tile_min: G3857) -> Vec<Feature> {
    let kind = row
        .class
        .as_deref()
        .or(row.subtype.as_deref())
        .unwrap_or("");
    let asset_id = if MAJOR_BUILDING_CLASSES.contains(&kind) {
        Ground::BuildingMajor as GroundId
    } else {
        Ground::Building as GroundId
    };
    let rank = Rank::by_area(Layer::Building, row.way_area, "", &row.gers_id);
    wkb_to_geometries(&row.geom_wkb)
        .iter()
        .filter_map(|geom| Feature::of(geom, tile_min, asset_id, rank.clone()))
        .collect()
}

/// What of a segment's part the ground shows: none of a tunnel or an indoor
/// passage, which show nothing from above, nor of a `water` segment, a ferry
/// route or a strait's label line, nor of a crossing, a line over a road the
/// road already shows; and not where a line draws it, cut square there, so
/// the paint ends where the line begins.
pub fn ways(row: &Part, way_ranges: &[WayRange], tile_min: G3857) -> Vec<Feature> {
    if row.subtype == "water"
        || row.tags.tunnel
        || row.tags.indoor
        || row.tags.role == Role::Crossing
    {
        return Vec::new();
    }
    // The lines' ranges are along the segment; take them into the part's own span.
    let [a0, b0] = row.between;
    let lined = merged(
        way_ranges
            .iter()
            .filter(|c| c.gers_id == row.gers_id && c.range[1] > a0 && c.range[0] < b0)
            .map(|c| c.range.map(|f| ((f - a0) / (b0 - a0)).clamp(0.0, 1.0)))
            .collect(),
        0.0,
    );
    let asset_id = segment_type(&row.subtype, row.class.as_deref());
    let rank = Rank::new(
        Layer::Road,
        paint_order(&row.subtype, row.class.as_deref(), row.tags.role),
        "",
        &row.gers_id,
    );
    let paint = |points: &[G3857], [a, b]: [f64; 2]| {
        let mut f = Feature::line(points, tile_min, asset_id, rank.clone())?;
        f.width = width_u(&row.subtype, row.class.as_deref(), row.tags.role);
        // Square where the segment is cut, round at its own ends.
        f.cut = [a > 0.0 || a0 > 0.0, b < 1.0 || b0 < 1.0];
        Some(f)
    };
    if lined.is_empty() {
        return paint(&row.points, [0.0, 1.0]).into_iter().collect();
    }
    let Some(line) = Line::of(&row.points) else {
        return Vec::new();
    };
    uncovered(&lined)
        .into_iter()
        .filter_map(|r| paint(&line.between(r[0], r[1]), r))
        .collect()
}

/// Every Base row with a Carto fill; an area layer paints larger areas first.
pub(crate) fn bases(row: &BaseRow, tile_min: G3857) -> Vec<Feature> {
    let (src, subtype, class) = (
        row.src.as_str(),
        row.subtype.as_deref().unwrap_or(""),
        row.class.as_deref().unwrap_or(""),
    );
    wkb_to_geometries(&row.geom_wkb)
        .iter()
        .filter_map(|geom| {
            let closed = closes(geom);
            let (asset_id, layer, feat) = match src {
                "water" => water_paint(subtype, class, closed),
                "land" => land_paint(class),
                "infrastructure" => infrastructure_paint(class, closed),
                // A land use line is a leisure track, which Carto draws from z17.
                _ if !closed => None,
                _ if subtype == "golf" => golf_paint(class).or_else(|| land_use_paint(class)),
                _ => land_use_paint(class),
            }?;
            let rank = match layer {
                // Carto's `aeroways` puts a runway over a taxiway.
                Layer::Aeroway => {
                    Rank::new(layer, i64::from(class == "runway"), feat, &row.gers_id)
                }
                Layer::WaterBarrier | Layer::Pier => {
                    Rank::new(layer, barrier_order(class, closed), feat, &row.gers_id)
                }
                _ => Rank::by_area(layer, row.way_area, feat, &row.gers_id),
            };
            let mut f = Feature::of(geom, tile_min, asset_id, rank)?;
            if !closed {
                f.width = line_width(src, class);
            }
            Some(f)
        })
        .collect()
}

/// The order in which ways paint, the later keeping the cell: OpenStreetMap Carto's `z` from
/// `roads_info` (openstreetmap-carto-flex.lua, CC0); other transport features rank under all.
fn paint_order(subtype: &str, class: Option<&str>, role: Role) -> i64 {
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
