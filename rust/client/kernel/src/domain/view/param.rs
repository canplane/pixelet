// pxl-kernel/domain/view/param.rs

//! What the eye observes and how finely the world is drawn for it.

use pxl_spec::space::coord::adapter::epsg4326::G4326;

/// Where the focus starts with no fix: Seoul City Hall. Also lights the first
/// frames and decides day or night before `focus` exists.
pub const FALLBACK_G4326: G4326 = [126.978, 37.5665, 0.0];

// ---- the drawn resolution ----

/// The least share of the display's pixels, each way, the world is drawn at.
pub const SCALE_MIN: f64 = 0.5;

/// The steps from whole down to `SCALE_MIN`, each a like share of the last:
/// two of them halve the pixels.
pub const SCALE_STEPS: i32 = 4;

/// The budgets a window of the load sums, in ms: what a step down waits
/// for.
pub const SCALE_WINDOW_MS: f64 = 500.0;

/// The windows in a row the step above must fit in before it is taken,
/// where the device reports GPU time.
pub const SCALE_UP_WINDOWS: u32 = 4;

/// A window within this of its budget, either way, holds the step.
pub const SCALE_BAND: f64 = 0.1;

/// The tightest budget: 60 Hz. Faster displays would trade sharpness for
/// frames the eye gains less from.
pub const SCALE_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// A change rebuilds the render targets, so the windows after one, a second
/// of them, are not judged.
pub const SCALE_SETTLE_WINDOWS: u32 = 2;

/// Where the device reports no GPU time, a window whose frames' gap averages
/// past this many budgets missed frames.
pub const SCALE_MISSED: f64 = 1.5;

/// Where the device reports no GPU time, the quiet windows before the first
/// probe up, 5 s, and the most a failed probe doubles them to, 60 s.
pub const SCALE_PROBE_WINDOWS: u32 = 10;
pub const SCALE_PROBE_WINDOWS_MAX: u32 = 120;

/// The windows after a probe up, 2 s, within which going over takes it back
/// and doubles the wait before the next.
pub const SCALE_TRUST_WINDOWS: u32 = 4;

/// The picker shows a case at thumbnail size: its buffer's long side, in pixels.
pub const CASE_LONG_SIDE_MAX: f64 = 256.0;

// ---- the projection ----

pub const NEAR: f64 = 0.1;

// ---- what is seen to move ----

/// The share of a camera grid cell a motion stepped a quantum at a time
/// carries what it moves a step, at the nearest drawn (`lens::seen_step_s`),
/// set by eye on the clouds: a twelfth of a cell, 1/1152 rad, is not seen
/// to move by itself, and a whole cell's step at once was seen as the
/// clouds stopping and jumping.
pub const SEEN_STEP_CELLS: f64 = 1.0 / 12.0;
