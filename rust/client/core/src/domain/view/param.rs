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
/// carries what it moves a step, at the nearest drawn (`lens::visible_step_s`),
/// set by eye on the clouds: a twelfth of a cell, 1/1152 rad, is not seen
/// to move by itself.
pub const VISIBLE_STEP_CELLS: f64 = 1.0 / 12.0;
