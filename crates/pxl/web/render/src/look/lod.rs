// pxl-render/look/lod.rs

//! The screen-space criterion the world's tiers are measured by, at the
//! reference view.

use std::sync::LazyLock;

/// What everything past the rule's last handoff shows.
pub const COARSEST: LodStep = 64;

/// Cells one box spans a side, a power of two.
pub type LodStep = u32;

/// A tier hands off when one of its cells would project to about this many
/// pixels: the old 192u ladder at 1080px and 50°, restated as screen-space error.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;
pub const REFERENCE_THRESHOLD_0: f64 = pxl_spec::space::lod::SPLIT;

/// The same projection factor the badges size with.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// Six pixels at the reference.
pub static TARGET_CELL_PX: LazyLock<f64> = LazyLock::new(|| px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0);
