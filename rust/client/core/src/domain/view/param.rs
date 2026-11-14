// pxl-core/domain/view/param.rs

//! The constants the view is tuned by.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

/// Where the pivot starts before the device reports a location: Seoul City
/// Hall, as longitude, latitude and height. The first frames are lit from
/// here too, and it decides day or night until the pivot exists.
pub(crate) const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

/// The near plane's distance from the eye, in world units.
pub(crate) const NEAR: f64 = 0.1;

/// How far a motion that is stepped a quantum at a time moves what it
/// carries in one step, as a share of the camera grid cell at the nearest
/// distance it is drawn (`lens::visible_step_s`). The value was chosen by
/// watching the clouds: a step of a twelfth of a cell, 1/1152 rad, is not
/// seen as a jump.
pub(crate) const VISIBLE_STEP_CELLS: f64 = 1.0 / 12.0;

/// A decor is drawn as a model while it spans at least this many camera grid
/// cells, and as an impostor below that.
pub(crate) const DECOR_MODEL_CELLS: f64 = 2.0;

/// A decor's impostor is not drawn when it spans less than this share of a
/// camera grid cell, because an instance that small reads as pixel noise. The
/// value was chosen by eye.
pub(crate) const DECOR_IMPOSTOR_CELLS: f64 = 0.25;
