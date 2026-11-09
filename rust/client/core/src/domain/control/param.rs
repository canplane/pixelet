// pxl-core/domain/control/param.rs

//! The tuned numbers of control: the camera's defaults and limits, how it
//! settles and how gestures and keys move it; how far the pointer picks; and
//! how long following and the sensors wait out a silence.

use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::zoom::units;
use pxl_spec::space::layer::lattice::GROUND;
use pxl_spec::space::layer::quadrant;

// ---- the camera ----

/// The least height, in units, of the orbit eye over the floor under it. It
/// is one cell, so the eye never sits in the cell that rests on the floor.
pub const EYE_CLEARANCE_U: f64 = 1.0;

pub const DEFAULT_DIST: f64 = 16.0;
pub const DEFAULT_AZIMUTH: f64 = 0.0;
/// 60° from straight down, which puts the eye 30° above the ground plane.
pub const DEFAULT_POLAR: f64 = core::f64::consts::FRAC_PI_3;
/// The distance from the pivot to either end of the dolly, counted in sides
/// of the camera grid cell drawn there. The grid draws a cell of side `s`
/// from 48·s to 96·s away (pxl-spec space/camera/grid.rs `split_dist`), and
/// 64·s lies a third of the way into that band.
pub const DOLLY_END_CELLS: f64 = 64.0;
const _: () = assert!(
    DOLLY_END_CELLS >= grid::CELLS_A_RAD / 2.0 && DOLLY_END_CELLS < grid::CELLS_A_RAD,
    "a dolly's limit lies in the band its end cell is drawn over"
);
/// The orbit's nearest distance from the pivot, in units. It is
/// `DOLLY_END_CELLS` sides of the 2^-5 cell the grid asks for there, about
/// 3.8 m at Seoul's latitude.
pub const MIN_DIST: f64 = 2.0;
/// How far from the pivot the eye counts as at the nearest, where it may
/// pass through what is there (camera/collision.rs, the Nearest rule).
pub const NEAREST_DIST: f64 = 1.25 * MIN_DIST;
/// The orbit's farthest distance: `DOLLY_END_CELLS` sides of the coarsest
/// cell any zoom holds, the cell of the pyramid's top. Looking straight down
/// from there, the grid never asks for a cell coarser than a zoom draws.
pub const MAX_DIST: f64 = DOLLY_END_CELLS * units(GROUND.cell_zoom(quadrant::MIN_ZOOM));

// ---- how the camera settles and keeps out of the ground ----

/// How close the first-person eye and facing come to their targets before
/// they stop easing. Each gap is smaller than a pixel at any distance.
pub const FIRST_PERSON_EYE_EPS: f64 = 0.001;
pub const FIRST_PERSON_FACING_EPS: f64 = 0.0001;
/// How close the orbit's pivot, distance, polar angle and azimuth come to
/// their targets before they stop easing. Each gap is smaller than a pixel
/// at any distance the view is seen across.
pub const ORBIT_PIVOT_EPS: f64 = 0.001;
pub const ORBIT_DIST_EPS: f64 = 0.001;
pub const ORBIT_POLAR_EPS: f64 = 0.0001;
pub const ORBIT_AZIMUTH_EPS: f64 = 0.0001;
/// The most samples a collision test takes along one step of the orbit
/// (camera/collision.rs `way_hit`). Testing only where a step lands would let
/// the camera tunnel, because the easing carries it through everything in
/// between.
pub const COLLISION_STEP_SAMPLES: u32 = 16;
/// How many times a climb steepens the pitch (camera/collision.rs
/// `steepened`). Steepening moves the eye, so the ground under the eye
/// changes too. Two passes leave less than a cell to go.
pub const CLIMB_PASSES: u32 = 2;

// ---- input ----

/// The share of the screen's height a drag crosses to tilt the camera by π.
/// It is stated against the screen to match the azimuth's arcball, which
/// turns one to one with the hand.
pub const TILT_SWEEP_FRACTION: f64 = 0.5;
pub const DOLLY_GAIN: f64 = 0.0022;
/// Within this radius of the screen's center, a rotate drag does not turn
/// the azimuth, because the hand's angle about the center (atan2) swings
/// wildly there.
pub const MIN_ROTATE_RADIUS_PX: f64 = 24.0;
pub const MIN_TWIST_DIST_PX: f64 = 24.0;
/// Once a rotate drag moves this far, its dominant axis locks for the rest
/// of the gesture. Without the lock, an off-center vertical drag would start
/// to spin the camera once the tilt reaches its limit. It is smaller than
/// `MIN_ROTATE_RADIUS_PX`, because it only has to tell vertical from
/// horizontal.
pub const AXIS_LOCK_THRESHOLD_PX: f64 = 12.0;
/// How far a pointer may move and still count as a tap. Real pointers
/// jitter a pixel or two on a still click.
pub const TAP_DRAG_THRESHOLD_PX: f64 = 4.0;
/// The share of the distance to the pivot that a held movement key pans
/// each second (`controller::pan_across`). It is a product decision. At any
/// distance and pitch, the pan crosses the width of a 16:9 view at the
/// pivot in about 3.4 s. Looking straight down, it crosses the view's height
/// in about 2 s.
pub const KEY_PAN_PER_S: f64 = 0.520_833_333_333_333_4;

// ---- smoothing ----

pub const ROT_DAMP: f64 = 10.0;
pub const CAM_DAMP: f64 = 10.0;
/// The rate a climb's pitch rises at. It is faster than the controls ease,
/// because a late rise leaves the eye briefly inside a hill.
pub const CLIMB_RISE_DAMP: f64 = 20.0;
/// The rate a climb's pitch relaxes at. It is slower than the controls
/// ease, so passing over a ridge does not bob the view.
pub const CLIMB_RELAX_DAMP: f64 = 5.0;
const _: () = assert!(
    CLIMB_RISE_DAMP > CAM_DAMP && CLIMB_RELAX_DAMP < CAM_DAMP,
    "the climb rises faster and relaxes slower than the controls ease"
);

/// The share of each new heading sample the followed azimuth takes. It
/// applies per sample, not per frame. Compass jitter, which is worse
/// indoors, passes the store's 1.5° dead zone, and this smooths it before it
/// reaches the azimuth.
pub const HEADING_SMOOTH_ALPHA: f64 = 0.25;

/// How long following outlasts a location that is not ready, in ms, set by
/// eye. A gap the location recovers from, such as a fix lost indoors or the
/// watch starting again, keeps following. A dead location lets following go
/// before the camera looks frozen behind a disabled button.
pub const SENSOR_GRACE_MS: f64 = 12000.0;

/// How far from the eye the pointer can pick, in units. It is a product
/// decision: what can be edited is a sphere about the observer.
pub const PICK_DIST_U: f64 = 32.0;

/// Orientation has no Permissions API, so a revoked permission is never
/// reported. A gap this long while tracking is taken as a loss instead. This
/// holds only where the sensor talks while still (`silence_is_loss`).
pub const WATCHDOG_TIMEOUT_MS: f64 = 8000.0;
