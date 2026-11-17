// pxl-core/domain/view/param.rs

//! The constants the view is tuned by.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

/// Where the pivot starts before the device reports a location: Seoul City
/// Hall, as longitude, latitude and height. The first frames are lit from
/// here too, and it decides day or night until the pivot exists.
pub(crate) const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

/// The near plane's distance from the eye, in world units.
pub(crate) const NEAR: f64 = 0.1;

/// The least turn of the camera the view takes, as what it moves on the
/// screen, in CSS pixels: half the reference pixel, which is a visual angle
/// and so stands for the spacing of the viewer's eye. A change the eye's grid
/// cannot tell apart is no change of what is seen (Nyquist).
pub(crate) const TURN_STEP_PX: f64 = 0.5;

/// How far a motion that is stepped a quantum at a time moves what it
/// carries in one step, as a share of the camera grid cell at the nearest
/// distance it is drawn (`lens::visible_step_s`). It is the motion's own
/// quantum, what time is sampled by, and not the view's (`TURN_STEP_PX`): a
/// still screen is drawn as often as this steps. The value was chosen by
/// watching the clouds: a step of a twelfth of a cell, 1/1152 rad, is not
/// seen as a jump.
pub(crate) const VISIBLE_STEP_CELLS: f64 = 1.0 / 12.0;

/// A decor's impostor is not drawn when it spans less than this share of a
/// camera grid cell, because an instance that small reads as pixel noise. The
/// value was chosen by eye. How far the clouds are drawn, whether a character
/// is seen and the size of the fixed ranges follow from it too.
pub(crate) const DECOR_IMPOSTOR_CELLS: f64 = 0.25;
