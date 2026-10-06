// pxl-geo-bake/terrain/pipeline/overture/policy.rs

//! Overture Buildings, Transportation and Base rows to the OpenStreetMap
//! Carto look each paints (gravitystorm/openstreetmap-carto 1cc4b89). An
//! Overture class is the value of the OSM tag it comes from, so it takes the
//! look, the layer and the `feature` Carto gives that tag (`landcover.mss`,
//! `water.mss`, `buildings.mss`, `roads.mss`, `project.mml`); a class Carto
//! draws no fill for paints nothing, and the land under it shows.

use pxl_spec::bake::constant::U_PER_CELL;
use pxl_spec::bake::deck::WIDEST;
use pxl_spec::bake::format::GroundCell;
use pxl_spec::cell::cube::cube_types::ground as g;
use pxl_spec::space::adapter::epsg3857::{from_db, u_per_m, G3857};

use crate::deck::Drawn;
use crate::segment::Segment;
use crate::terrain::pipeline::feature::{closes, Feature, Joins, Layer, Rank};
use crate::terrain::pipeline::overture::between::{cell_of, flag_ranges, length_of, merged, split_ranged};
use crate::terrain::pipeline::overture::role::Role;
use crate::terrain::pipeline::overture::width::{width_m, WayShape};
use crate::terrain::repository::{BaseRow, BuildingRow};
use crate::wkb::wkb_to_geometries;

/// A way's look: Carto's `highway` and `railway` values, Overture's
/// `unclassified` the residential road it is drawn as, `path` and `steps`
/// footways.
pub fn segment_type(subtype: &str, class: Option<&str>) -> GroundCell {
    let c = class.unwrap_or("");
    if subtype == "rail" {
        return match c {
            "subway" | "light_rail" | "tram" | "monorail" => g::subway,
            _ => g::rail,
        };
    }
    match c {
        "motorway" => g::motorway,
        "trunk" => g::trunk,
        "primary" => g::primary,
        "secondary" => g::secondary,
        "tertiary" => g::tertiary,
        "residential" | "unclassified" => g::residential_road,
        "living_street" => g::living_street,
        "service" => g::service,
        "pedestrian" => g::pedestrian,
        "track" => g::track,
        "cycleway" => g::cycleway,
        "bridleway" => g::bridleway,
        "footway" | "path" | "steps" => g::footway,
        _ => g::road,
    }
}

/// One-way ways of these classes are carriageways, not one-way streets.
const THROUGH_CLASSES: [&str; 5] = ["motorway", "trunk", "primary", "secondary", "tertiary"];

/// Carto's `building-major-fill`: a place of worship, a terminal or a station.
const MAJOR_BUILDING_CLASSES: [&str; 9] = ["cathedral", "chapel", "church", "mosque", "shrine", "synagogue", "temple", "train_station", "transportation"];

/// Nothing of a way under these shows from above. Not `is_disused` (the rails
/// remain) nor `is_covered` (the buildings paint over it).
pub const UNSEEN_FLAGS: &[&str] = &["is_tunnel", "is_indoor", "is_under_construction", "is_abandoned"];

pub const BORE_FLAGS: &[&str] = &["is_tunnel"];

pub const LINK_FLAGS: &[&str] = &["is_link"];

pub const DECK_FLAGS: &[&str] = &["is_bridge"];

/// What a Base row paints: its look, Carto's layer, and Carto's `feature`.
type Paint = (GroundCell, Layer, &'static str);

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
        "railway" => (g::industrial, L, "landuse_railway"),
        "works" => (g::industrial, L, "man_made_works"),
        "construction" => (g::construction, L, "landuse_construction"),
        "brownfield" => (g::construction, L, "landuse_brownfield"),
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
        "forest" => (g::forest, L, "landuse_forest"),
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
        "airfield" => (g::transportation_area, L, "aeroway_aerodrome"),
        "pedestrian" | "plaza" => (g::pedestrian, RoadArea, "highway_pedestrian"),
        _ => return None,
    })
}

/// `natural`, as Overture's land classes carry it; Carto draws a wetland by
/// its `wetland` tag, which Overture drops, so it is the grass most of them are.
fn land_paint(class: &str) -> Option<Paint> {
    use Layer::Landcover as L;
    Some(match class {
        "wood" | "forest" => (g::forest, L, "natural_wood"),
        "bare_rock" => (g::bare_ground, L, "natural_bare_rock"),
        "scree" => (g::bare_ground, L, "natural_scree"),
        "shingle" => (g::bare_ground, L, "natural_shingle"),
        "sand" => (g::sand, L, "natural_sand"),
        "beach" => (g::beach, L, "natural_beach"),
        "heath" => (g::heath, L, "natural_heath"),
        "scrub" => (g::scrub, L, "natural_scrub"),
        "grassland" => (g::grass, L, "natural_grassland"),
        "wetland" => (g::grass, L, "natural_wetland"),
        "glacier" => (g::glacier, Layer::Water, "natural_glacier"),
        _ => return None,
    })
}

/// Water areas but what Carto draws as landcover in water's color or as a
/// shoal; a strait's label line paints nothing.
fn water_paint(class: &str) -> Option<Paint> {
    use Layer::Landcover as L;
    Some(match class {
        "strait" => return None,
        "shoal" => (g::beach, L, "natural_shoal"),
        "salt_pond" => (g::water, L, "landuse_salt_pond"),
        "swimming_pool" => (g::water, L, "leisure_swimming_pool"),
        _ => (g::water, Layer::Water, ""),
    })
}

/// The infrastructure classes with a Carto fill, the only ones read. An
/// aeroway's area is `highway-area-fill`'s, its line the `aeroways` layer's.
pub const INFRASTRUCTURE: &[(&str, Paint)] = {
    use Layer::{Landcover as L, RoadArea};
    &[
        ("parking", (g::parking, L, "amenity_parking")),
        ("bicycle_parking", (g::parking, L, "amenity_bicycle_parking")),
        ("motorcycle_parking", (g::parking, L, "amenity_motorcycle_parking")),
        ("apron", (g::apron, L, "aeroway_apron")),
        ("airport", (g::transportation_area, L, "aeroway_aerodrome")),
        ("international_airport", (g::transportation_area, L, "aeroway_aerodrome")),
        ("municipal_airport", (g::transportation_area, L, "aeroway_aerodrome")),
        ("regional_airport", (g::transportation_area, L, "aeroway_aerodrome")),
        ("private_airport", (g::transportation_area, L, "aeroway_aerodrome")),
        ("military_airport", (g::transportation_area, L, "aeroway_aerodrome")),
        ("bus_station", (g::transportation_area, L, "amenity_bus_station")),
        ("ferry_terminal", (g::transportation_area, L, "amenity_ferry_terminal")),
        ("railway_station", (g::industrial, L, "railway_station")),
        ("substation", (g::power, L, "power_substation")),
        ("generator", (g::power, L, "power_generator")),
        ("plant", (g::power, L, "power_plant")),
        ("camp_site", (g::campsite, L, "tourism_camp_site")),
        ("platform", (g::platform, RoadArea, "railway_platform")),
        ("runway", (g::aeroway, RoadArea, "aeroway_runway")),
        ("taxiway", (g::aeroway, RoadArea, "aeroway_taxiway")),
        ("helipad", (g::aeroway, RoadArea, "aeroway_helipad")),
    ]
};

fn infrastructure_paint(class: &str, closed: bool) -> Option<Paint> {
    let &(_, (look, layer, feature)) = INFRASTRUCTURE.iter().find(|(c, _)| *c == class)?;
    Some((look, if look == g::aeroway && !closed { Layer::Aeroway } else { layer }, feature))
}

pub fn buildings(row: &BuildingRow, tile_min: G3857) -> Vec<Feature> {
    let kind = row.class.as_deref().or(row.subtype.as_deref()).unwrap_or("");
    let look = if MAJOR_BUILDING_CLASSES.contains(&kind) { g::building_major } else { g::building };
    let rank = Rank::by_area(Layer::Building, row.way_area, "", &row.gers_id);
    wkb_to_geometries(&row.geom_wkb).iter().filter_map(|geom| Feature::of(geom, tile_min, look, rank.clone())).collect()
}

/// A segment's parts the ground shows: not where an unseen flag holds or a
/// line draws it, cut square there, so the paint ends where the line begins;
/// none of a `water` segment, a ferry route or a strait's label line, nor of a
/// crossing, a line over a road the road already shows.
pub fn ways(row: &Segment, drawn: &[Drawn], tile_min: G3857) -> Vec<Feature> {
    if row.subtype == "water" {
        return Vec::new();
    }
    let unseen = flag_ranges(row.flags.as_deref(), UNSEEN_FLAGS, cell_of(length_of(&row.geom_wkb)));
    let lined = merged(drawn.iter().filter(|c| c.gers_id == row.gers_id).map(|c| c.range).collect(), 0.0);
    let look = segment_type(&row.subtype, row.class.as_deref());
    let cell_per_m = u_per_m(from_db(tile_min[1])) / U_PER_CELL;
    wkb_to_geometries(&row.geom_wkb)
        .into_iter()
        .flat_map(|g| split_ranged(g, [&unseen, &lined]))
        .filter(|([unseen, lined], ..)| !unseen && !lined)
        .filter_map(|(_, [a, b], geom)| {
            let shape = WayShape::at(row.columns(), (a + b) / 2.0);
            if shape.role == Role::Crossing {
                return None;
            }
            let rank = Rank::new(Layer::Road, paint_order(&row.subtype, row.class.as_deref(), shape.role), "", &row.gers_id);
            let mut f = Feature::of(&geom, tile_min, look, rank)?;
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

/// Every Base row with a Carto fill; an area layer paints larger areas first.
pub fn bases(row: &BaseRow, tile_min: G3857) -> Vec<Feature> {
    let class = row.class.as_deref().unwrap_or("");
    wkb_to_geometries(&row.geom_wkb)
        .iter()
        .filter_map(|geom| {
            let (look, layer, feature) = match row.source.as_str() {
                "water" => water_paint(class),
                "land" => land_paint(class),
                "infrastructure" => infrastructure_paint(class, closes(geom)),
                _ => land_use_paint(class),
            }?;
            let rank = match layer {
                // Carto's `aeroways` puts a runway over a taxiway.
                Layer::Aeroway => Rank::new(layer, i64::from(class == "runway"), feature, &row.gers_id),
                _ => Rank::by_area(layer, row.way_area, feature, &row.gers_id),
            };
            Feature::of(geom, tile_min, look, rank)
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
