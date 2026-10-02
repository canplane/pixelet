// pxl-geo-bake/deck/clearance.rs

//! Deck clearances. OSM's `layer` is an order, not metres, so a deck is as
//! high as what passes under it needs; the numbers are Arnis's (Apache-2.0),
//! one block a metre.

/// Over a road.
pub const ROAD: f64 = 6.0;
/// Over a footway, path or cycleway.
pub const PATH: f64 = 5.0;
/// Over a railway: the overhead wire stands six above the track.
pub const RAIL: f64 = 8.0;
/// Over a river's, canal's or lake's water: Arnis's.
pub const RIVER: f64 = 4.0;

/// Extra per metre of water spanned: puts a deck over the Han (1 km) about
/// fourteen metres up, where its bridges are.
pub const RIVER_PER_M: f64 = 0.01;
pub const RIVER_MOST: f64 = 15.0;
/// Over a stream, ditch or drain.
pub const STREAM: f64 = 2.0;
/// Over a lower deck, from its top.
pub const STACKED: f64 = 6.0;

/// Ours, not a precedent's: a city's ramps run 6 to 8 percent, and the
/// approach is graded with the bridge, so the climb has a real length.
pub const MAX_SLOPE: f64 = 0.08;

/// About two cells; a kilometre of deck is two hundred points.
pub const STEP: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Under {
    Road,
    Path,
    Rail,
    River,
    Stream,
}

impl Under {
    pub fn headroom(self, span: f64) -> f64 {
        match self {
            Under::Road => ROAD,
            Under::Path => PATH,
            Under::Rail => RAIL,
            Under::River => (RIVER + RIVER_PER_M * span).min(RIVER_MOST),
            Under::Stream => STREAM,
        }
    }

    pub fn of_way(subtype: &str, class: Option<&str>) -> Under {
        match (subtype, class.unwrap_or("")) {
            ("rail", _) => Under::Rail,
            (_, "footway" | "path" | "steps" | "cycleway" | "pedestrian" | "bridleway") => Under::Path,
            _ => Under::Road,
        }
    }

    /// None for the sea, whose height the ground already is, and a name-only feature.
    pub fn of_water(subtype: &str, class: Option<&str>) -> Option<Under> {
        match (subtype, class.unwrap_or("")) {
            ("ocean", _) | ("physical", _) => None,
            ("stream", _) | (_, "ditch" | "drain" | "stream") => Some(Under::Stream),
            _ => Some(Under::River),
        }
    }
}
