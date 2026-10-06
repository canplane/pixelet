// pxl-geo-bake/terrain/pipeline/overture/width.rs

//! A way's width in cells, the same on the ground and on a deck: Carto's at
//! z16 (`roads.mss`, gravitystorm/openstreetmap-carto 1cc4b89), the zoom whose
//! pixel is a cell (`web_zoom_of(CELL)`), its casing included. Carto reads no
//! measured width, nor a way's lanes, and neither does the bake.

use crate::terrain::pipeline::overture::role::Role;

/// `@footway-width-z16`: a sidewalk is a footway.
const FOOTWAY: f64 = 1.3;

pub fn width_cells(subtype: &str, class: Option<&str>, role: Role) -> f64 {
    let c = class.unwrap_or("");
    if subtype == "rail" {
        // Each railway's line at z16 (`railway_*`).
        return match c {
            "tram" => 1.5,
            "light_rail" | "narrow_gauge" | "funicular" | "subway" => 2.0,
            "monorail" => 3.0,
            _ => 4.0,
        };
    }
    match role {
        Role::Sidewalk | Role::Crossing => return FOOTWAY,
        // `@minor-service-width-z16`: a driveway, a parking aisle.
        Role::Lane => return 2.0,
        Role::Link => match c {
            "motorway" | "trunk" | "primary" => return 7.8,
            "secondary" | "tertiary" => return 7.0,
            _ => {}
        },
        Role::Way => {}
    }
    // `@*-width-z16`, or z15's where z16 sets none.
    match c {
        "motorway" | "trunk" | "primary" | "secondary" | "tertiary" => 10.0,
        "residential" | "unclassified" | "living_street" | "pedestrian" => 6.0,
        "track" => 1.5,
        "steps" => 3.0,
        "bridleway" => 1.2,
        "footway" | "path" => FOOTWAY,
        "cycleway" => 0.9,
        _ => 3.5,
    }
}
