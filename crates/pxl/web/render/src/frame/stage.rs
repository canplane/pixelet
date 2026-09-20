// pxl-render/frame/stage.rs

//! The frame's uniforms as the kernel decides them, in one lane of doubles at
//! a fixed address: the camera and the root frame, the LOD fade, and what the
//! settings switch on. The kernel writes it after every frame's turn
//! (crates/pxl/web/kernel/src/world/frame/stage.rs); the renderer reads it in place,
//! converts the projections to WebGPU's depth range, lays the numbers into its
//! own uniform buffers and draws; it decides nothing.

use xpute_runtime::global::Global;

/// A cube edge's shade where it borders open air, while edge shading is on,
/// and the shaded band's width.
pub const EDGE_STRENGTH: f64 = 0.3;
pub const EDGE_WIDTH: f64 = 0.03;
/// How much of the ambient light a face loses where it meets a wall, at the
/// wall itself, while the corner occlusion is on. It is a Rust constant and
/// not one of chunk.wgsl's, where the rest of the shading's numbers are,
/// because the kernel decides whether the mode is on and writes this or
/// zero into the lane; the shader cannot see a setting. It scales the warm
/// tint by the same amount as the darkening, so a face reads no more
/// coloured than it reads dark. How far the shading reaches from the wall,
/// and how it falls away over that distance, are chunk.wgsl's AO_REACH and
/// occlusion_of.
pub const AO_STRENGTH: f64 = 0.2;

pub const STAGE_WORDS: usize = 96;

/// Where each value sits in the lane.
#[allow(non_snake_case)]
pub mod STAGE_AT {
    /// The badges' view: the view matrix through the root frame (16).
    pub const BADGE_VIEW: usize = 0;
    /// Camera to GL clip (16).
    pub const PROJ: usize = 16;
    /// Root frame to GL clip (16).
    pub const LOCAL_VIEW_PROJ: usize = 32;
    /// View frame to camera (16), for the sky.
    pub const VIEW_MATRIX: usize = 48;
    /// The eye in the root frame, where the meshes are (3).
    pub const EYE_LOCAL: usize = 64;
    /// The eye in the view frame (3).
    pub const EYE: usize = 67;
    /// Where the root frame's origin stands (3): the shadow aims at its negation.
    pub const TRANSLATION: usize = 70;
    /// The viewport in CSS pixels (2).
    pub const VIEWPORT: usize = 73;
    pub const FOV: usize = 75;
    pub const NEAR: usize = 76;
    pub const FAR: usize = 77;
    pub const FADE_START: usize = 78;
    pub const FADE_END: usize = 79;
    pub const LOD_CELL: usize = 80;
    pub const EDGE_STRENGTH: usize = 81;
    pub const LOW_SPEC: usize = 82;
    pub const EDGE_WIDTH: usize = 83;
    pub const RIM_INTENSITY: usize = 84;
    pub const SHADOWS: usize = 85;
    pub const SKY_RADIUS: usize = 86;
    /// MSAA is the renderer's targets and every pipeline's sample count: a
    /// change rebuilds both over the same canvas.
    pub const MSAA: usize = 87;
    pub const AO_STRENGTH: usize = 88;
    /// The screen-space occlusion (gpu/ao.rs): 1 to run it, 0 not to.
    pub const SCREEN_AO: usize = 89;
}

static STAGE: Global<[f64; STAGE_WORDS]> = Global::new();

/// The lane, for the kernel to write the frame's uniforms into.
pub fn stage_lane_mut() -> &'static mut [f64; STAGE_WORDS] {
    STAGE.get(|| [0.0; STAGE_WORDS])
}

/// The lane, for the renderer to lay the frame's uniforms out from.
pub fn stage_lane() -> &'static [f64; STAGE_WORDS] {
    stage_lane_mut()
}
