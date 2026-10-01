// pxl-geo-bake/deck/clearance.rs

//! What a deck has to clear, and how steeply it may rise to clear it.
//!
//! A deck's height is not its layer times a step: OSM's `layer` is an order
//! among what crosses, and says nothing of metres. A deck is as high as what
//! passes under it needs, which is how Arnis, which builds bridges from OSM
//! into Minecraft, solves them (its bridge module, Apache-2.0); the numbers
//! here are its, in metres, its block being one.

/// A deck's top over the ground where a road passes under it.
pub const ROAD: f64 = 6.0;
/// … where a footway, a path or a cycleway does.
pub const PATH: f64 = 5.0;
/// … where a railway does: the overhead wire stands six above the track.
pub const RAIL: f64 = 8.0;
/// … over a river's, canal's or lake's water, the least: Arnis's.
pub const RIVER: f64 = 4.0;

/// … and how much higher for every metre of water a deck spans, up to
/// `RIVER_MOST`. A wide river is navigable and its bridges stand clear of
/// its boats: a metre a hundred metres of water puts a deck over the Han,
/// a kilometre across, some fourteen metres up, about where its bridges'
/// decks are, and leaves a stream's crossing at Arnis's four.
pub const RIVER_PER_M: f64 = 0.01;
pub const RIVER_MOST: f64 = 15.0;
/// … over a stream's, ditch's or drain's.
pub const STREAM: f64 = 2.0;
/// … over a lower deck it crosses, from that deck's top.
pub const STACKED: f64 = 6.0;

/// The steepest a deck climbs, or a bore descends, rise over run, from where
/// it meets the ground. Pixelet's, not a precedent's: a city's ramps run
/// about 6 to 8 percent, and with the approach graded with the bridge
/// (approach.rs) the climb has the length a real one has. Arnis, which grades
/// the bridge way alone, has to allow one to one.
pub const MAX_SLOPE: f64 = 0.08;

/// How far apart the solve samples a deck, in metres: about two cells, so
/// a deck's line bends where its ground and what it clears change, and a
/// kilometre of deck is two hundred points.
pub const STEP: f64 = 5.0;

/// Piers stand this far apart along a deck, a short span's length.
pub const PIER_STEP: f64 = 30.0;
/// A pier stands only under a deck this far over the ground, room for one
/// under its slab.
pub const PIER_MIN_RISE: f64 = 4.0;
/// … and no nearer than this to a road or a railway that passes under.
pub const PIER_OFF_WAY: f64 = 8.0;

/// What passes under a deck, by how much it asks the deck to clear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Under {
    Road,
    Path,
    Rail,
    River,
    Stream,
}

impl Under {
    /// The height over the ground there the deck's top must reach, where
    /// it spans `span` metres of what it clears.
    pub fn headroom(self, span: f64) -> f64 {
        match self {
            Under::Road => ROAD,
            Under::Path => PATH,
            Under::Rail => RAIL,
            Under::River => (RIVER + RIVER_PER_M * span).min(RIVER_MOST),
            Under::Stream => STREAM,
        }
    }

    /// Whether a pier keeps off it: a way is driven or walked through, and
    /// water is where a river bridge's piers stand.
    pub fn keeps_piers_off(self) -> bool {
        matches!(self, Under::Road | Under::Path | Under::Rail)
    }

    /// A transportation segment's, by its subtype and class.
    pub fn of_way(subtype: &str, class: Option<&str>) -> Under {
        match (subtype, class.unwrap_or("")) {
            ("rail", _) => Under::Rail,
            (_, "footway" | "path" | "steps" | "cycleway" | "pedestrian" | "bridleway") => Under::Path,
            _ => Under::Road,
        }
    }

    /// A base water feature's, by its subtype and class, or none for one a
    /// deck is not raised over — the sea, whose height the ground already
    /// is, and what is only a name.
    pub fn of_water(subtype: &str, class: Option<&str>) -> Option<Under> {
        match (subtype, class.unwrap_or("")) {
            ("ocean", _) | ("physical", _) => None,
            ("stream", _) | (_, "ditch" | "drain" | "stream") => Some(Under::Stream),
            _ => Some(Under::River),
        }
    }
}
