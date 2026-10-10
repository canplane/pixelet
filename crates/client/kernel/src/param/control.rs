// pxl-kernel/param/control.rs

//! The camera's defaults, gestures and smoothing.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

/// Where the focus starts with no fix: Seoul City Hall. Also lights the first
/// frames and decides day or night before `focus` exists.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

// ---- the camera ----

/// Degrees from the ground plane.
pub const DEFAULT_TILT_DEG: f64 = 30.0;

/// The orbit eye's least height over its floor: one cell, so the eye is never
/// in the cell its floor tops.
pub const EYE_CLEARANCE_U: f64 = 1.0;

pub const DEFAULT_DIST: f64 = 16.0;
pub const DEFAULT_AZIMUTH: f64 = 0.0;
/// 0 = straight down, PI / 2 = horizontal; (90 - DEFAULT_TILT_DEG)°.
pub const DEFAULT_POLAR: f64 = 1.0471975511965976;
/// Felt-right against the scale bar: about 0.18 m on a 1080px viewport at
/// Seoul's latitude.
pub const MIN_DIST: f64 = 2.0;
/// Looking straight down from here the ground wants step 2^11, which the
/// pyramid's top zoom draws (about 24 km on the scale bar); past it no zoom
/// is coarse enough.
pub const MAX_DIST: f64 = 262144.0;

// ---- input ----

/// Stated against the screen, not as rad/px, to match azimuth's 1:1 arcball:
/// a fixed rate of -0.012 rad/px moved tilt six times faster than azimuth, and
/// its settle after release read as the gesture still being held.
pub const TILT_SWEEP_FRACTION: f64 = 0.5;
pub const DOLLY_GAIN: f64 = 0.0022;
/// atan2 swings wildly near the screen center; below this, azimuth freezes.
pub const MIN_ROTATE_RADIUS_PX: f64 = 24.0;
pub const MIN_TWIST_DIST_PX: f64 = 24.0;
/// Past this displacement the dominant axis locks for the gesture, or an
/// off-center vertical drag spins once tilt saturates. Smaller than
/// MIN_ROTATE_RADIUS_PX: it only needs to tell vertical from horizontal.
pub const AXIS_LOCK_THRESHOLD_PX: f64 = 12.0;
/// Real pointers jitter a pixel or two on a still click; gating taps on any
/// movement swallowed them.
pub const TAP_DRAG_THRESHOLD_PX: f64 = 4.0;
/// A held movement key pans this many of the camera grid's cells at the
/// focus a second: a wide screen crossed in about two seconds at any zoom,
/// as 600 CSS pixels a second did on the reference view.
pub const KEY_PAN_CELLS_PER_S: f64 = 50.0;

// ---- smoothing ----

pub const ROT_DAMP: f64 = 10.0;
pub const POS_DAMP: f64 = 10.0;
pub const CAM_DAMP: f64 = 10.0;
