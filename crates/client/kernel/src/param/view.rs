// pxl-kernel/param/view.rs

//! What the eye observes and how finely the world is drawn for it.

// ---- the drawn resolution ----

/// Device pixels per CSS pixel the world is drawn at: the display's, no
/// finer than labels' sheets are drawn, so a label is never shown smaller
/// than drawn, and under load a share of them (view/frame/resolution.rs).
pub const RESOLUTION_MAX: f64 = pxl_spec::space::camera::grid::RATIO_MAX;
const _: () = assert!(RESOLUTION_MAX <= pxl_render::gpu::sheet::SHEET_SCALE, "a label's sheet is drawn at least as fine as the world");

/// The least share of the display's pixels, each way, the world is drawn at.
pub const SCALE_MIN: f64 = 0.5;

/// The cast within this of the frame's budget, either way, holds the scale.
pub const SCALE_BAND: f64 = 0.1;

/// The picker shows a case at thumbnail size: its buffer's long side, in pixels.
pub const CASE_LONG_SIDE_MAX: f64 = 256.0;

// ---- the projection ----

pub const FOV: f64 = 50.0;
pub const NEAR: f64 = 0.1;
