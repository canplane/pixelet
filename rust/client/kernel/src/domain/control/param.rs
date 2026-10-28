// pxl-kernel/domain/control/param.rs

//! The camera's defaults, gestures and smoothing.

// ---- the camera ----

/// The orbit eye's least height over its floor: one cell, so the eye is never
/// in the cell its floor tops.
pub const EYE_CLEARANCE_U: f64 = 1.0;

pub const DEFAULT_DIST: f64 = 16.0;
pub const DEFAULT_AZIMUTH: f64 = 0.0;
/// From straight down (0) toward horizontal (PI / 2): 60°, the eye 30° above
/// the ground plane.
pub const DEFAULT_POLAR: f64 = core::f64::consts::FRAC_PI_3;
/// Felt-right against the scale bar: about 0.19 m on a 1080px viewport at
/// Seoul's latitude.
pub const MIN_DIST: f64 = 2.0;
/// Looking straight down from here the ground wants step 2^11, which the
/// pyramid's top zoom draws (about 24 km on the scale bar); past it no zoom
/// is coarse enough.
pub const MAX_DIST: f64 = 262144.0;

// ---- the camera's settling and the ground it keeps to ----

/// The free eye's and its look's settling, each sub-pixel at any distance.
pub const FREE_EYE_EPS: f64 = 0.001;
pub const FREE_LOOK_EPS: f64 = 0.0001;
/// The orbit's center, distance, polar and azimuth settling, each sub-pixel
/// at any distance the view is looked across.
pub const ORBIT_CENTER_EPS: f64 = 0.001;
pub const ORBIT_DIST_EPS: f64 = 0.001;
pub const ORBIT_POLAR_EPS: f64 = 0.0001;
pub const ORBIT_AZIMUTH_EPS: f64 = 0.0001;
/// Testing only where a step lands lets the camera tunnel, since the orbit's
/// step damps through everything between.
pub const GATE_STEP_SAMPLES: u32 = 16;
/// Steepening moves the eye and so the ground under it; two passes settle it,
/// the remainder being smaller than a cell.
pub const CLIMB_PASSES: u32 = 2;

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
/// A held movement key pans this share of the distance to the focus a
/// second (`controller::pan_across`): a product number, a wide view crossed
/// in about two seconds at any distance, 10-12's fifty camera-grid cells of
/// 96 a distance. Not the grid's cells, whose count a distance moves with
/// how finely the world is drawn and not with how fast a view should pan.
pub const KEY_PAN_PER_S: f64 = 50.0 / 96.0;

// ---- smoothing ----

pub const ROT_DAMP: f64 = 10.0;
pub const CAM_DAMP: f64 = 10.0;
/// Faster than the controls ease: late to rise is the eye briefly inside a hill.
pub const CLIMB_RISE_DAMP: f64 = 20.0;
/// Slower than the controls ease, so passing over a ridge does not bob the view.
pub const CLIMB_RELAX_DAMP: f64 = 5.0;
