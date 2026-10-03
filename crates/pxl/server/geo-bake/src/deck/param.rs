// pxl-geo-bake/deck/param.rs

//! The solve's knobs; clearances and shells are pxl-spec's `bake/deck.rs`.

use pxl_spec::bake::deck::DECK_THICKNESS_M;

// ---- clear height over water, in ground metres ----

// Arnis's deck height over the water, less the slab under it
pub const RIVER_M: f64 = 4.0 - DECK_THICKNESS_M; // a river, canal or lake
pub const STREAM_M: f64 = 2.0 - DECK_THICKNESS_M; // a stream, ditch or drain
                                                  // pixelet's, no source found
pub const RIVER_PER_M: f64 = 0.01; // more a metre of water spanned: a kilometre's river is crossed high
pub const RIVER_MOST_M: f64 = 15.0; // what that rise stops at

// ---- grade ----

/// UNECE AGR Annex II III.2.1's steepest road grade (60 km/h); a need takes
/// what grade it must. One grade for every way: a fallback, not a rule of each.
pub const PREFERRED_SLOPE: f64 = 0.08;

// ---- along a line, in ground metres ----

pub const STEP_M: f64 = 5.0; // a solve's sample: a kilometre is two hundred points
pub const AT_END_M: f64 = 1.0; // a crossing this near a piece's end is its joint
