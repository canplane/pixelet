// pxl-kernel/param/view.rs

//! What the eye observes and how finely the world is drawn for it.

// ---- what refinement observes ----

/// The camera grid the estimator's patches are cast at, as a τ: its cell is
/// about 10°, 77 rays on a 16:9 screen at 50°. An eighth of the side cast 135
/// rays for goals within 1% of these over rolling ground (measured 2026-09-28).
pub const OBSERVED_TAU: f64 = 16.0;

/// How far a patch's last answer may drift, as a share of a patch, before it
/// is cast again.
pub const REUSE_SHIFT: f64 = 0.25;

// ---- the camera grid ----

/// The most camera grid cells a screen holds: past it a cell grows. It bounds
/// what a screen draws (chunks, triangles) up front, where the arena would
/// coarsen only once its slots ran short. About a 1780 × 1110 window at
/// twelve-pixel cells; a phone holds a fifth of it.
pub const GRID_CELLS_MAX: f64 = 10_000.0;

// ---- the drawn resolution ----

/// Device pixels per CSS pixel the world is drawn at: one at least, and no
/// finer than labels' sheets are drawn, so a label is never shown smaller
/// than drawn.
pub const RESOLUTION_MIN: f64 = 1.0;
pub const RESOLUTION_MAX: f64 = pxl_render::gpu::sheet::SHEET_SCALE;

/// The picker shows a case at thumbnail size: its buffer's long side, in pixels.
pub const CASE_LONG_SIDE_MAX: f64 = 256.0;

pub const RESOLUTION_STEP: f64 = 0.25;

/// Over budget this long before a step down, so one slow frame is not taken for load.
pub const RESOLUTION_OVER_HOLD_MS: f64 = 500.0;

pub const RESOLUTION_ROOM_HOLD_MS: f64 = 2000.0;

/// A step up adds at most (2.0 / 1.75)² ≈ 1.3× fill, so half the budget leaves room.
pub const RESOLUTION_ROOM: f64 = 0.5;

/// Interval in budgets that counts as a missed frame when the device reports no GPU time.
pub const RESOLUTION_MISSED: f64 = 1.5;

/// First probe up, and the longest backoff between probes, when the device
/// reports no GPU time.
pub const RESOLUTION_PROBE_HOLD_MS: f64 = 5000.0;
pub const RESOLUTION_PROBE_HOLD_MAX_MS: f64 = 60_000.0;

pub const RESOLUTION_PROBE_TRUST_MS: f64 = 2000.0;

/// The tightest budget: 60 Hz. Faster displays would trade sharpness for
/// frames the eye gains less from.
pub const RESOLUTION_BUDGET_FLOOR_MS: f64 = 1000.0 / 60.0;

/// A ratio change rebuilds the render targets, so changes are spaced out.
pub const RESOLUTION_CHANGE_GAP_MS: f64 = 1000.0;

// ---- the projection ----

pub const FOV: f64 = 50.0;
pub const NEAR: f64 = 0.1;

/// The far clip against the reach: past it, so ground at the reach is whole.
pub const FAR_CLIP: f64 = 1.5;
