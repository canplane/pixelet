// pxl-geo-bake/deck/param.rs

//! The solve's knobs. A way's clear height and a structure's shell are
//! pxl-spec's (`bake/deck.rs`), shared with the client; these are the server's.

// ---- clear height over water, in ground metres: Arnis's ----

pub const RIVER_M: f64 = 4.0; // over a river's, canal's or lake's water
pub const RIVER_PER_M: f64 = 0.01; // more a metre of water spanned: a kilometre's river is crossed high
pub const RIVER_MOST_M: f64 = 15.0; // what that rise stops at
pub const STREAM_M: f64 = 2.0; // over a stream, ditch or drain

// ---- grade ----

/// A city ramp's 6 to 8 %. A preference only: a structure's need is met at
/// whatever grade it takes.
pub const PREFERRED_SLOPE: f64 = 0.08;

// ---- along a line, in ground metres ----

pub const STEP_M: f64 = 5.0; // a solve's sample: a kilometre is two hundred points
pub const AT_END_M: f64 = 1.0; // a crossing this near a piece's end is its joint
pub const SIDE_GAP_M: f64 = 2.0; // Arnis's: parallel decks this near are one structure

/// A slip road leaving at an angle is another way, not an approach.
pub const APPROACH_TURN_COS: f64 = 0.5; // 60°
