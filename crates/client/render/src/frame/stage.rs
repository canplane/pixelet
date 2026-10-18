// pxl-render/frame/stage.rs

//! The frame's uniforms as the kernel decides them, in one lane of doubles at a
//! fixed address; the renderer reads it in place.

use xpute_guest::global::Global;

pub const STAGE_WORDS: usize = STAGE_AT::CLOUDS + 1;

/// Where each value sits in the lane.
#[allow(non_snake_case)]
pub mod STAGE_AT {
    /// The view matrix through the root frame (16).
    pub const BADGE_VIEW: usize = 0;
    /// Camera to the device's clip, as `mat4::perspective` makes it (16).
    pub const PROJ: usize = 16;
    /// Eye-relative, so the matrix holds no eye translation for a point's to cancel against (16).
    pub const EYE_VIEW_PROJ: usize = 32;
    /// The eye, in the world's doubles (3).
    pub const EYE: usize = 48;
    /// The viewport in CSS pixels (2).
    pub const VIEWPORT: usize = EYE + 3;
    pub const FOV: usize = VIEWPORT + 2;
    pub const LOD_CELL: usize = FOV + 1;
    pub const EDGE_STRENGTH: usize = LOD_CELL + 1;
    pub const LOW_SPEC: usize = EDGE_STRENGTH + 1;
    pub const EDGE_WIDTH: usize = LOW_SPEC + 1;
    pub const RIM_INTENSITY: usize = EDGE_WIDTH + 1;
    pub const SCREEN_AO: usize = RIM_INTENSITY + 1;
    /// How much of the clouds shows, none to all.
    pub const CLOUDS: usize = SCREEN_AO + 1;
}

static STAGE: Global<[f64; STAGE_WORDS]> = Global::new();

/// The lane: the kernel writes it a turn, the renderer reads it a frame.
pub fn stage_lane() -> &'static mut [f64; STAGE_WORDS] {
    STAGE.get(|| [0.0; STAGE_WORDS])
}
