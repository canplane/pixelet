// pxl-core/domain/control/param.rs

//! The tuned numbers of control: the camera's defaults and limits, how it
//! settles and how gestures and keys move it; how far the pointer picks; how
//! long following outlasts a location that is not ready; and how far the
//! pivot goes before it is off the device.

use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::zoom::{MIN_ZOOM, units};
use pxl_spec::space::layer::lattice::GROUND;

// ---- the camera ----

/// The least height, in units, of the orbit eye over the floor under it. It
/// is one cell, so the eye never sits in the cell that rests on the floor.
pub(crate) const EYE_CLEARANCE_U: f64 = 1.0;

pub(crate) const DEFAULT_DIST: f64 = 16.0;
pub(crate) const DEFAULT_AZIMUTH: f64 = 0.0;
/// 60° from straight down, which puts the eye 30° above the ground plane.
pub(crate) const DEFAULT_POLAR: f64 = core::f64::consts::FRAC_PI_3;
/// The distance from the pivot to either end of the dolly, counted in sides
/// of the camera grid cell drawn there. The grid draws a cell of side `s`
/// from 48·s to 96·s away (pxl-spec space/camera/grid.rs `split_dist`), and
/// 64·s lies a third of the way into that band.
pub(crate) const DOLLY_END_CELLS: f64 = 64.0;
const _: () = assert!(
    DOLLY_END_CELLS >= grid::CELLS_A_RAD as f64 / 2.0 && DOLLY_END_CELLS < grid::CELLS_A_RAD as f64,
    "a dolly's limit lies in the band its end cell is drawn over"
);
/// The orbit's nearest distance from the pivot, in units. It is
/// `DOLLY_END_CELLS` sides of the 2^-5 cell the grid asks for there, about
/// 3.8 m at Seoul's latitude.
pub(crate) const MIN_DIST: f64 = 2.0;
/// How far from the pivot the eye counts as at the nearest, where it may
/// pass through what is there (camera/orbit.rs, the Nearest rule).
pub(crate) const NEAREST_DIST: f64 = 1.25 * MIN_DIST;
/// The orbit's farthest distance: `DOLLY_END_CELLS` sides of the coarsest
/// cell any zoom holds, the cell of the pyramid's top. Looking straight down
/// from there, the grid never asks for a cell coarser than a zoom draws.
pub(crate) const MAX_DIST: f64 = DOLLY_END_CELLS * units(GROUND.cell_zoom(MIN_ZOOM));

// ---- how the camera settles and keeps out of the ground ----

/// The most samples a collision test takes along one step of the orbit
/// (camera/orbit.rs `way_hit`). Testing only where a step lands would let
/// the camera tunnel, because the easing carries it through everything in
/// between.
pub(crate) const COLLISION_STEP_SAMPLE_CNT: u32 = 16;
/// How many times a climb steepens the pitch (camera/orbit.rs
/// `steepened`). Steepening moves the eye, so the ground under the eye
/// changes too. Two passes leave less than a cell to go.
pub(crate) const CLIMB_PASS_CNT: u32 = 2;

// ---- input ----

/// The share of the distance to the pivot that a held movement key pans
/// each second (`controller::pan_across`). It is a product decision. At any
/// distance and pitch, the pan crosses the width of a 16:9 view at the
/// pivot in about 3.4 s. Looking straight down, it crosses the view's height
/// in about 2 s.
pub(crate) const KEY_PAN_PER_S: f64 = 0.520_833_333_333_333_4;

// ---- smoothing ----

pub(crate) const ROT_DAMP: f64 = 10.0;
pub(crate) const CAM_DAMP: f64 = 10.0;
/// The rate a climb's pitch rises at. It is faster than the controls ease,
/// because a late rise leaves the eye briefly inside a hill.
pub(crate) const CLIMB_RISE_DAMP: f64 = 20.0;
/// The rate a climb's pitch relaxes at. It is slower than the controls
/// ease, so passing over a ridge does not bob the view.
pub(crate) const CLIMB_RELAX_DAMP: f64 = 5.0;
const _: () = assert!(
    CLIMB_RISE_DAMP > CAM_DAMP && CLIMB_RELAX_DAMP < CAM_DAMP,
    "the climb rises faster and relaxes slower than the controls ease"
);

/// How long following outlasts a location that is not ready, in ms, set by
/// eye. A gap the location recovers from, such as a fix lost indoors or the
/// watch starting again, keeps following. A dead location lets following go
/// before the camera looks frozen behind a disabled button.
pub(crate) const SENSOR_GRACE_MS: f64 = 12000.0;

/// The pivot goes off the device past `OFF_CENTER_U` across the ground and
/// comes back within `ON_CENTER_U`, so a pivot near the edge does not flicker
/// between the two (navigation/off_center.rs).
pub(crate) const OFF_CENTER_U: f64 = 10.0;
pub(crate) const ON_CENTER_U: f64 = 3.0;
const _: () = assert!(
    ON_CENTER_U < OFF_CENTER_U,
    "the pivot comes back nearer than it goes off"
);

/// How far from the eye the pointer can pick, in units. It is a product
/// decision: what can be edited is a sphere about the eye.
pub(crate) const PICK_DIST_U: f64 = 32.0;
