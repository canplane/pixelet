// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings, Transportation and Base rows to the OpenStreetMap
//! Carto look each paints (gravitystorm/openstreetmap-carto 1cc4b89). An
//! Overture class is the value of the OSM tag it comes from, so it takes the
//! look, the layer and the `feature` Carto gives that tag (`landcover.mss`,
//! `water.mss`, `buildings.mss`, `roads.mss`, `project.mml`); a class Carto
//! draws no fill for paints nothing, and the land under it shows.

use pxl_spec::asset::cube::ground as g;
use pxl_spec::bake::tile::terrain::GroundId;
use pxl_spec::space::coord::adapter::epsg3857::G3857;

use crate::segment::Part;
use crate::separation::Drawn;
use crate::terrain::pipeline::feature::{closes, Feature, Layer, Rank};
use crate::terrain::pipeline::overture::between::{merged, uncovered, Line};
use crate::terrain::pipeline::overture::role::Role;
use crate::terrain::pipeline::overture::width::{carto_u, width_u};
use crate::terrain::repository::{BaseRow, BuildingRow};
use crate::wkb::wkb_to_geometries;

/// A way's look: Carto's `highway` and `railway` values, its white roads one
/// look told apart by width, `path` and `steps` footways.
pub fn segment_type(subtype: &str, class: Option<&str>) -> GroundId {
    let c = class.unwrap_or("");
    if subtype == "rail" {
        return match c {
            "tram" => g::tram,
            "light_rail" | "narrow_gauge" | "funicular" => g::light_rail,
            "subway" => g::subway,
            "monorail" => g::monorail,
            _ => g::rail,
        };
    }
    match c {
        "motorway" => g::motorway,
        "trunk" => g::trunk,
        "primary" => g::primary,
        "secondary" => g::secondary,
        "tertiary" | "residential" | "unclassified" | "service" => g::residential_road,
        "living_street" => g::living_street,
        "pedestrian" => g::pedestrian,
        "track" => g::track,
        "cycleway" => g::cycleway,
        "bridleway" => g::bridleway,
        "footway" | "path" | "steps" => g::footway,
        _ => g::road,
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

/// What a Base row paints: its look, Carto's layer, and Carto's `feature`.
type Paint = (GroundId, Layer, &'static str);

/// `landuse`, `leisure`, `amenity` and the like, as Overture's land use
/// classes carry them.
fn land_use_paint(class: &str) -> Option<Paint> {
    use Layer::{Landcover as L, RoadArea};
    Some(match class {
        "residential" => (g::residential, L, "landuse_residential"),
        "garages" => (g::garages, L, "landuse_garages"),
        "retail" => (g::retail, L, "landuse_retail"),
        "commercial" => (g::commercial, L, "landuse_commercial"),
        "industrial" => (g::industrial, L, "landuse_industrial"),
        "works" => (g::industrial, L, "man_made_works"),
        "landfill" => (g::landfill, L, "landuse_landfill"),
        "quarry" => (g::quarry, L, "landuse_quarry"),
        "farmland" => (g::farmland, L, "landuse_farmland"),
        "greenhouse_horticulture" => (g::farmland, L, "landuse_greenhouse_horticulture"),
        "farmyard" => (g::farmyard, L, "landuse_farmyard"),
        "orchard" => (g::orchard, L, "landuse_orchard"),
        "vineyard" => (g::orchard, L, "landuse_vineyard"),
        "plant_nursery" => (g::orchard, L, "landuse_plant_nursery"),
        "allotments" => (g::allotments, L, "landuse_allotments"),
        "cemetery" => (g::cemetery, L, "landuse_cemetery"),
        "grave_yard" => (g::cemetery, L, "amenity_grave_yard"),
        "meadow" => (g::grass, L, "landuse_meadow"),
        "grass" => (g::grass, L, "landuse_grass"),
        "village_green" => (g::grass, L, "landuse_village_green"),
        "flowerbed" => (g::grass, L, "landuse_flowerbed"),
        "garden" => (g::grass, L, "leisure_garden"),
        "park" => (g::park, L, "leisure_park"),
        "recreation_ground" => (g::leisure, L, "landuse_recreation_ground"),
        "playground" => (g::leisure, L, "leisure_playground"),
        "dog_park" => (g::leisure, L, "leisure_dog_park"),
        "stadium" => (g::leisure, L, "leisure_stadium"),
        "water_park" => (g::leisure, L, "leisure_water_park"),
        "pitch" => (g::pitch, L, "leisure_pitch"),
        "track" => (g::pitch, L, "leisure_track"),
        "golf_course" => (g::campsite, L, "leisure_golf_course"),
        "camp_site" => (g::campsite, L, "tourism_camp_site"),
        "religious" => (g::place_of_worship, L, "landuse_religious"),
        "school" => (g::societal_amenities, L, "amenity_school"),
        "college" => (g::societal_amenities, L, "amenity_college"),
        "university" => (g::societal_amenities, L, "amenity_university"),
        "kindergarten" => (g::societal_amenities, L, "amenity_kindergarten"),
        "hospital" => (g::societal_amenities, L, "amenity_hospital"),
        "clinic" => (g::societal_amenities, L, "amenity_clinic"),
        "salt_pond" => (g::water, L, "landuse_salt_pond"),
        "pedestrian" | "plaza" => (g::pedestrian, RoadArea, "highway_pedestrian"),
        _ => return None,
    })
}

/// `golf.mss`, by the `golf` tag; a water hazard has no fill.
fn golf_paint(class: &str) -> Option<Paint> {
    use Layer::Landcover as L;
    Some(match class {
        "tee" => (g::grass, L, "golf_tee"),
        "fairway" => (g::grass, L, "golf_fairway"),
        "rough" => (g::grass, L, "golf_rough"),
        "driving_range" => (g::grass, L, "golf_driving_range"),
        "green" => (g::pitch, L, "golf_green"),
        "bunker" => (g::sand, L, "golf_bunker"),
        _ => return None,
    })
}

/// `natural`, as Overture's land classes carry it; Carto fills a wetland by
/// its `wetland` tag, which Overture drops, and one without it not at all.
/// The only land classes read: the rest, an island's coast of 585,169 points
/// among them, paint nothing.
pub const LAND: &[(&str, Paint)] = {
    use Layer::Landcover as L;
    &[
        ("wood", (g::forest, L, "natural_wood")),
        ("forest", (g::forest, L, "landuse_forest")),
        ("bare_rock", (g::bare_ground, L, "natural_bare_rock")),
        ("scree", (g::bare_ground, L, "natural_scree")),
        ("shingle", (g::bare_ground, L, "natural_shingle")),
        ("sand", (g::sand, L, "natural_sand")),
        ("beach", (g::beach, L, "natural_beach")),
        ("heath", (g::heath, L, "natural_heath")),
        ("scrub", (g::scrub, L, "natural_scrub")),
        ("grassland", (g::grass, L, "natural_grassland")),
        ("glacier", (g::glacier, Layer::Water, "natural_glacier")),
    ]
};

fn land_paint(class: &str) -> Option<Paint> {
    LAND.iter()
        .find(|(c, _)| *c == class)
        .map(|&(_, paint)| paint)
}

/// `water-lines`' waterways, each its fill's width at z16 (`water.mss`: a
/// canal is a stream by `@canal-scale-factor`, a ditch and a drain take z14's
/// until z17); a line of another class has no fill.
const WATERWAYS: [(&str, f64); 5] = [
    ("river", 8.0),
    ("canal", 4.2),
    ("stream", 3.0),
    ("ditch", 2.0),
    ("drain", 2.0),
];

/// A waterway line's width in u, if Carto draws one of its class.
pub fn waterway_width(class: &str) -> Option<f64> {
    WATERWAYS
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, px)| carto_u(*px))
}

/// Water areas but what Carto draws as landcover in water's color or as a
/// shoal; a physical feature or a spring (a bay, a strait) has no fill.
fn water_paint(subtype: &str, class: &str, closed: bool) -> Option<Paint> {
    use Layer::Landcover as L;
    if !closed {
        return waterway_width(class).map(|_| (g::water, Layer::WaterLine, ""));
    }
    Some(match (subtype, class) {
        (_, "shoal") => (g::beach, L, "natural_shoal"),
        (_, "salt_pond") => (g::water, L, "landuse_salt_pond"),
        (_, "swimming_pool") => (g::water, L, "leisure_swimming_pool"),
        ("physical" | "spring", _) => return None,
        _ => (g::water, Layer::Water, ""),
    })
}

/// The infrastructure classes with a Carto fill, the only ones read. An
/// aeroway's area is `highway-area-fill`'s, its line the `aeroways` layer's.
pub const INFRASTRUCTURE: &[(&str, Paint)] = {
    use Layer::{Landcover as L, RoadArea};
    &[
        ("parking", (g::parking, L, "amenity_parking")),
        (
            "bicycle_parking",
            (g::parking, L, "amenity_bicycle_parking"),
        ),
        (
            "motorcycle_parking",
            (g::parking, L, "amenity_motorcycle_parking"),
        ),
        ("apron", (g::apron, L, "aeroway_apron")),
        ("airport", (g::transportation_area, L, "aeroway_aerodrome")),
        (
            "international_airport",
            (g::transportation_area, L, "aeroway_aerodrome"),
        ),
        (
            "municipal_airport",
            (g::transportation_area, L, "aeroway_aerodrome"),
        ),
        (
            "regional_airport",
            (g::transportation_area, L, "aeroway_aerodrome"),
        ),
        (
            "private_airport",
            (g::transportation_area, L, "aeroway_aerodrome"),
        ),
        (
            "military_airport",
            (g::transportation_area, L, "aeroway_aerodrome"),
        ),
        (
            "bus_station",
            (g::transportation_area, L, "amenity_bus_station"),
        ),
        (
            "ferry_terminal",
            (g::transportation_area, L, "amenity_ferry_terminal"),
        ),
        ("railway_station", (g::industrial, L, "railway_station")),
        ("subway_station", (g::industrial, L, "railway_station")),
        ("substation", (g::power, L, "power_substation")),
        ("generator", (g::power, L, "power_generator")),
        ("plant", (g::power, L, "power_plant")),
        ("platform", (g::platform, RoadArea, "railway_platform")),
        ("runway", (g::aeroway, RoadArea, "aeroway_runway")),
        ("taxiway", (g::aeroway, RoadArea, "aeroway_taxiway")),
        ("helipad", (g::aeroway, RoadArea, "aeroway_helipad")),
        (
            "seaplane_airport",
            (g::transportation_area, L, "aeroway_aerodrome"),
        ),
        ("dam", (g::dam, Layer::WaterBarrier, "waterway_dam")),
        (
            "weir",
            (g::breakwater, Layer::WaterBarrier, "waterway_weir"),
        ),
        (
            "breakwater",
            (g::breakwater, Layer::Pier, "man_made_breakwater"),
        ),
        ("pier", (g::land, Layer::Pier, "man_made_pier")),
        (
            "cutline",
            (g::grass, Layer::LandcoverLine, "man_made_cutline"),
        ),
    ]
};

/// Carto draws these as lines alone.
const LINE_ONLY: [&str; 2] = ["weir", "cutline"];

fn infrastructure_paint(class: &str, closed: bool) -> Option<Paint> {
    let &(_, (look, layer, feature)) = INFRASTRUCTURE.iter().find(|(c, _)| *c == class)?;
    if closed && LINE_ONLY.contains(&class) {
        return None;
    }
    Some((
        look,
        if look == g::aeroway && !closed {
            Layer::Aeroway
        } else {
            layer
        },
        feature,
    ))
}

/// A runway's, the widest line Carto draws at z16, in its pixels.
pub const WIDEST_LINE: f64 = 24.0;

/// A Base line's width in u: Carto's at z16, as a way's
/// (`overture/width.rs`), its casing included.
fn line_width(source: &str, class: &str) -> f64 {
    if source == "water" {
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

pub fn buildings(row: &BuildingRow, tile_min: G3857) -> Vec<Feature> {
    let kind = row
        .class
        .as_deref()
        .or(row.subtype.as_deref())
        .unwrap_or("");
    let look = if MAJOR_BUILDING_CLASSES.contains(&kind) {
        g::building_major
    } else {
        g::building
    };
    let rank = Rank::by_area(Layer::Building, row.way_area, "", &row.gers_id);
    wkb_to_geometries(&row.geom_wkb)
        .iter()
        .filter_map(|geom| Feature::of(geom, tile_min, look, rank.clone()))
        .collect()
}

/// What of a segment's part the ground shows: none of a tunnel or an indoor
/// passage, which show nothing from above, nor of a `water` segment, a ferry
/// route or a strait's label line, nor of a crossing, a line over a road the
/// road already shows; and not where a line draws it, cut square there, so
/// the paint ends where the line begins.
pub fn ways(row: &Part, drawn: &[Drawn], tile_min: G3857) -> Vec<Feature> {
    if row.subtype == "water"
        || row.tags.tunnel
        || row.tags.indoor
        || row.tags.role == Role::Crossing
    {
        return Vec::new();
    }
    // the lines' ranges are the segment's; in the part's own
    let [a0, b0] = row.span;
    let lined = merged(
        drawn
            .iter()
            .filter(|c| c.gers_id == row.gers_id && c.range[1] > a0 && c.range[0] < b0)
            .map(|c| c.range.map(|f| ((f - a0) / (b0 - a0)).clamp(0.0, 1.0)))
            .collect(),
        0.0,
    );
    let look = segment_type(&row.subtype, row.class.as_deref());
    let rank = Rank::new(
        Layer::Road,
        paint_order(&row.subtype, row.class.as_deref(), row.tags.role),
        "",
        &row.gers_id,
    );
    let paint = |points: &[G3857], [a, b]: [f64; 2]| {
        let mut f = Feature::line(points, tile_min, look, rank.clone())?;
        f.width = width_u(&row.subtype, row.class.as_deref(), row.tags.role);
        // square where the segment is cut, round at its own ends
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
pub fn bases(row: &BaseRow, tile_min: G3857) -> Vec<Feature> {
    let (source, subtype, class) = (
        row.source.as_str(),
        row.subtype.as_deref().unwrap_or(""),
        row.class.as_deref().unwrap_or(""),
    );
    wkb_to_geometries(&row.geom_wkb)
        .iter()
        .filter_map(|geom| {
            let closed = closes(geom);
            let (look, layer, feature) = match source {
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
                    Rank::new(layer, i64::from(class == "runway"), feature, &row.gers_id)
                }
                Layer::WaterBarrier | Layer::Pier => {
                    Rank::new(layer, barrier_order(class, closed), feature, &row.gers_id)
                }
                _ => Rank::by_area(layer, row.way_area, feature, &row.gers_id),
            };
            let mut f = Feature::of(geom, tile_min, look, rank)?;
            if !closed {
                f.width = line_width(source, class);
            }
            Some(f)
        })
        .collect()
}

/// Later keeps the cell. OpenStreetMap Carto's `z` from `roads_info`
/// (openstreetmap-carto-flex.lua, CC0); other transport features rank under all.
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
