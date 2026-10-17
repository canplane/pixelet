// pxl-kernel/param/view.rs

//! What the eye observes and how finely the world is drawn for it.

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

// ---- the projection ----

pub const FOV: f64 = 50.0;
pub const NEAR: f64 = 0.1;

/// The far clip against the reach: past it, so ground at the reach is whole.
pub const FAR_CLIP: f64 = 1.5;
