// pxl-render/frame/stage.rs

//! The frame's uniforms as the kernel decides them, in one lane of doubles at a
//! fixed address; the renderer reads it in place.

use xpute_runtime::global::Global;

pub const STAGE_WORDS: usize = 80;

/// Where each value sits in the lane.
#[allow(non_snake_case)]
pub mod STAGE_AT {
    /// The view matrix through the root frame (16).
    pub const BADGE_VIEW: usize = 0;
    /// Camera to GL clip (16).
    pub const PROJ: usize = 16;
    /// Eye-relative, so the matrix holds no eye translation for a point's to cancel against (16).
    pub const EYE_VIEW_PROJ: usize = 32;
    pub const EYE_LOCAL: usize = 48;
    pub const GLOBE_ANCHOR: usize = 51;
    /// The viewport in CSS pixels (2).
    pub const VIEWPORT: usize = 54;
    pub const FOV: usize = 56;
    pub const NEAR: usize = 57;
    pub const FAR: usize = 58;
    pub const LOD_CELL: usize = 59;
    pub const EDGE_STRENGTH: usize = 60;
    pub const LOW_SPEC: usize = 61;
    pub const EDGE_WIDTH: usize = 62;
    pub const RIM_INTENSITY: usize = 63;
    pub const SHADOWS: usize = 64;
    pub const AO_STRENGTH: usize = 65;
    pub const SCREEN_AO: usize = 66;
    /// East and north, in units (2); the clouds are anchored to it.
    pub const ORIGIN: usize = 67;
    /// Device pixels per CSS pixel the world is drawn at this frame.
    pub const RESOLUTION: usize = 69;
}

static STAGE: Global<[f64; STAGE_WORDS]> = Global::new();

pub fn stage_lane_mut() -> &'static mut [f64; STAGE_WORDS] {
    STAGE.get(|| [0.0; STAGE_WORDS])
}

pub fn stage_lane() -> &'static [f64; STAGE_WORDS] {
    stage_lane_mut()
}
