// pxl-core/domain/view/param.rs

//! The constants the view is tuned by.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

/// Where the pivot starts before the device reports a location: Seoul City
/// Hall, as longitude, latitude and height. The first frames are lit from
/// here too, and it decides day or night until the pivot exists.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

// ---- the projection ----

/// The near plane's distance from the eye, in world units.
pub const NEAR: f64 = 0.1;

// ---- what is seen to move ----

/// How far a motion that is stepped a quantum at a time moves what it
/// carries in one step, as a share of the camera grid cell at the nearest
/// distance it is drawn (`lens::visible_step_s`). The value was chosen by
/// watching the clouds: a step of a twelfth of a cell, 1/1152 rad, is not
/// seen as a jump.
pub const VISIBLE_STEP_CELLS: f64 = 1.0 / 12.0;
