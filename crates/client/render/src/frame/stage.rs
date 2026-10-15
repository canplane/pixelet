// pxl-render/frame/stage.rs

//! The frame's uniforms as the kernel decides them, in one lane of doubles at a
//! fixed address; the renderer reads it in place.

use xpute_guest::global::Global;

pub const STAGE_WORDS: usize = STAGE_AT::RAY_STEPS + 1;

/// Where each value sits in the lane.
#[allow(non_snake_case)]
pub mod STAGE_AT {
    /// The view matrix through the root frame (16).
    pub const BADGE_VIEW: usize = 0;
    /// Camera to GL clip (16).
    pub const PROJ: usize = 16;
    /// Eye-relative, so the matrix holds no eye translation for a point's to cancel against (16).
    pub const EYE_VIEW_PROJ: usize = 32;
    /// The eye and the sea under it, in the world's doubles (3 each).
    pub const EYE: usize = 48;
    pub const GLOBE_ANCHOR: usize = 51;
    /// The viewport in CSS pixels (2).
    pub const VIEWPORT: usize = 54;
    pub const FOV: usize = 56;
    pub const LOD_CELL: usize = 57;
    pub const EDGE_STRENGTH: usize = 58;
    pub const LOW_SPEC: usize = 59;
    pub const EDGE_WIDTH: usize = 60;
    pub const RIM_INTENSITY: usize = 61;
    pub const SHADOWS: usize = 62;
    pub const AO_STRENGTH: usize = 63;
    pub const SCREEN_AO: usize = 64;
    /// How much of the clouds shows, none to all.
    pub const CLOUDS: usize = 65;
    /// 1 to colour each ray's cell by the nodes its descent asked, not by
    /// what it met.
    pub const RAY_STEPS: usize = 66;
}

static STAGE: Global<[f64; STAGE_WORDS]> = Global::new();

pub fn stage_lane_mut() -> &'static mut [f64; STAGE_WORDS] {
    STAGE.get(|| [0.0; STAGE_WORDS])
}

pub fn stage_lane() -> &'static [f64; STAGE_WORDS] {
    stage_lane_mut()
}
