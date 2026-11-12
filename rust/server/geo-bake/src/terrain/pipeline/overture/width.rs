// pxl-geo-bake/terrain/pipeline/overture/width.rs

//! A way's width, the same on the ground and on a bridge: Carto's at z16
//! (`roads.mss`, gravitystorm/openstreetmap-carto 1cc4b89) in its pixels, its
//! casing included. Carto reads no measured width, nor a way's lanes, and
//! neither does the bake.

use pxl_spec::space::coord::adapter::epsg3857::cell_zoom_of;
use pxl_spec::space::coord::zoom::units;

use crate::terrain::pipeline::overture::role::Role;

/// The web zoom Carto's widths are read at.
const CARTO_ZOOM: i32 = 16;

/// A Carto width, in its z16 pixels, in u: the one place one becomes a length.
pub const fn carto_u(px: f64) -> f64 {
    px * units(cell_zoom_of(CARTO_ZOOM))
}

/// A runway's, the widest line Carto draws at z16, in its pixels.
pub(crate) const WIDEST_LINE: f64 = 24.0;

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
pub(crate) fn waterway_width(class: &str) -> Option<f64> {
    WATERWAYS
        .iter()
        .find(|(c, _)| *c == class)
        .map(|(_, px)| carto_u(*px))
}

/// `@footway-width-z16`: a sidewalk is a footway.
const FOOTWAY: f64 = 1.3;

pub fn width_u(subtype: &str, class: Option<&str>, role: Role) -> f64 {
    carto_u(width_px(subtype, class, role))
}

fn width_px(subtype: &str, class: Option<&str>, role: Role) -> f64 {
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
