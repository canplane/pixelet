// pxl-core/domain/view/param.rs

//! What the eye observes.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

/// Where the pivot starts with no fix: Seoul City Hall. Also lights the first
/// frames and decides day or night before `pivot` exists.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

// ---- the projection ----

pub const NEAR: f64 = 0.1;

// ---- what is seen to move ----

/// The share of a camera grid cell a motion stepped a quantum at a time
/// carries what it moves a step, at the nearest drawn (`lens::seen_step_s`),
/// set by eye on the clouds: a twelfth of a cell, 1/1152 rad, is not seen
/// to move by itself.
pub const SEEN_STEP_CELLS: f64 = 1.0 / 12.0;

// ---- what a thing spans: a model, an impostor or culled ----

/// A decor kind is drawn as a model while it spans at least this share of a
/// camera grid cell, an impostor below it: set by eye at one cell, moved to
/// two by the user's decision of 2026-11-06, so a model is drawn down to
/// about 22 px.
pub const DECOR_MODEL_CELLS: f64 = 2.0;

/// Under this share of a camera grid cell a decor kind's impostor is not drawn:
/// set by eye, below it an instance is pixel noise.
pub const DECOR_IMPOSTOR_CELLS: f64 = 0.25;
