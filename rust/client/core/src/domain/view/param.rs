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

// ---- what a thing spans: its lod ----

/// A decor is drawn as a model while it spans at least this many camera grid
/// cells, and as an impostor below that. At two cells a model is drawn down
/// to about 22 px.
pub const DECOR_MODEL_CELLS: f64 = 2.0;

/// A decor's impostor is not drawn when it spans less than this share of a
/// camera grid cell, because an instance that small reads as pixel noise. The
/// value was chosen by eye.
pub const DECOR_IMPOSTOR_CELLS: f64 = 0.25;
