// pxl-core/domain/control/param.rs

//! The tuned numbers of control: the camera's defaults and limits, how it
//! settles and how gestures and keys move it; and how long tracking outlasts
//! a location that is not ready.

use crate::domain::smooth::Flight;
use pxl_spec::space::camera::grid;
use pxl_spec::space::coord::zoom::{MIN_ZOOM, units};
use pxl_spec::space::layer::lattice::GROUND;

// ---- the camera ----

/// The least height, in units, of the orbit eye over the floor under it. It
/// is one cell, so the eye never sits in the cell that rests on the floor.
/// At the nearest the eye stands lower than that, where the camera has it, and is
/// kept only out of the floor (camera/orbit.rs `climb_rise`).
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
/// How far from the pivot the eye counts as at the nearest, where the climb
/// draws it through what is there (camera/orbit.rs `at_climb`).
pub(crate) const NEAREST_DIST: f64 = 1.25 * MIN_DIST;
/// The orbit's farthest distance: `DOLLY_END_CELLS` sides of the coarsest
/// cell any zoom holds, the cell of the pyramid's top. Looking straight down
/// from there, the grid never asks for a cell coarser than a zoom draws.
pub(crate) const MAX_DIST: f64 = DOLLY_END_CELLS * units(GROUND.cell_zoom(MIN_ZOOM));

// ---- how the orbit keeps out of the ground ----

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

/// How the orbit flies to a place it is sent to (domain/smooth.rs `fly`),
/// apart from what a gesture asks. The way is van Wijk and Nuij's, flown
/// between views as wide as the camera's distance, and what the way widens
/// the view by is how high the flight carries the camera. Its curve sets
/// that against how far it goes: between views `w` wide, a way `u` long tops
/// at `hypot(w, curve² / 2 · u) − w` high. So a flight to a place beside it
/// is level and a longer one is higher with its length, with no step
/// between. 0.63 makes the top a fifth of a long way, set by eye; their
/// study has 1.42, where the top shows the whole way. The cruise is set by
/// eye: 4 is about six of the view's heights a second across the ground.
/// The rate is twice the one the facing eases at. The view gets no wider
/// than the camera's farthest distance.
pub(crate) const FLIGHT: Flight = Flight {
    curve: 0.63,
    cruise: 4.0,
    rate: 2.0 * FACING_DAMP,
    cap: MAX_DIST,
};

/// The rate the camera's facing eases at, a rotation and a tilt alike, per
/// second (smooth.rs `smoothing_factor`).
pub(crate) const FACING_DAMP: f64 = 10.0;
/// The rate the orbit's distance eases at, per second.
pub(crate) const DIST_DAMP: f64 = 10.0;
/// The rate a climb's pitch rises at. It is faster than the facing eases,
/// because a late rise leaves the eye briefly inside a hill.
pub(crate) const CLIMB_RISE_DAMP: f64 = 20.0;
/// The rate a climb's pitch relaxes at. It is slower than the facing eases, so
/// passing over a ridge does not bob the view.
pub(crate) const CLIMB_RELAX_DAMP: f64 = 5.0;
const _: () = assert!(
    CLIMB_RISE_DAMP > FACING_DAMP && CLIMB_RELAX_DAMP < FACING_DAMP,
    "the climb rises faster and relaxes slower than the facing eases"
);

/// How long tracking outlasts a location that is not ready, in ms, set by
/// eye. A gap the location recovers from, such as a fix lost indoors or the
/// watch starting again, keeps tracking. A dead location lets tracking go
/// before the camera looks frozen behind a disabled button.
pub(crate) const SENSOR_GRACE_MS: f64 = 12000.0;
