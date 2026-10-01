// pxl-render/look/lod.rs

//! The screen-space criterion the world's tiers are measured by, at the
//! reference view. Which level and cube a chunk is drawn in is the kernel's
//! rule (kernel world/pyramid/domain.rs, from pxl-spec space/lod.rs
//! `split_distance`), and whatever keeps step
//! with the tiers reads it there;
//! what stays here is the reference itself, and decor's target cell, the
//! same criterion applied to a kind's extent.

use std::sync::LazyLock;

/// The coarsest step a chunk's mesh is drawn at: its cells 64 a side of one
/// box, what everything past the rule's last handoff shows.
pub const COARSEST: LodStep = 64;

/// A mesh's step: how many of a chunk's cells one box spans a side, a power
/// of two.
pub type LodStep = u32;

/// The criterion: a tier hands off to the next when one of its cells would
/// project to about this many pixels on screen.
///
/// Screen-space error, the ordinary rule for chunked terrain LOD, rather than
/// a table of world distances. A table is this same rule frozen at one
/// viewport and one field of view, and it cannot say what should happen when
/// either moves — which is why the value below is not a fresh one: it is the
/// ladder that was already here (192u for the finest tier at 1080px and 50
/// degrees), restated. A change of reasoning, not a retune.
///
/// Every screen-space criterion in the world — a cell's tier (the kernel's
/// split distance), a decor kind by its extent — is this reference, so they
/// move together. The distance is the lattice's (pxl-spec space/lod.rs
/// `SPLIT`), since the pages a level's faults can name are derived from it
/// before any screen is known.
pub const REFERENCE_VIEWPORT_H: f64 = 1080.0;
pub const REFERENCE_FOV_DEG: f64 = 50.0;
pub const REFERENCE_THRESHOLD_0: f64 = pxl_spec::space::lod::SPLIT;

/// Pixels a world unit covers at one unit of distance — the same projection
/// factor the badges size their own screen measurements with.
pub fn px_per_unit(viewport_h: f64, fov_deg: f64) -> f64 {
    viewport_h / (2.0 * libm::tan((fov_deg * core::f64::consts::PI) / 360.0))
}

/// Six pixels at the reference. For whoever applies the same criterion to
/// something other than a cell — decor, by its extent.
pub static TARGET_CELL_PX: LazyLock<f64> = LazyLock::new(|| px_per_unit(REFERENCE_VIEWPORT_H, REFERENCE_FOV_DEG) / REFERENCE_THRESHOLD_0);
