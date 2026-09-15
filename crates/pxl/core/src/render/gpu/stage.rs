// pxl_core/render/gpu/stage.rs

//! The frame's uniforms as the kernel decides them: the camera and the root
//! frame from the view (module/world/frame/view.rs), the LOD fade, and what the
//! settings switch on — written into one lane of doubles at a fixed address
//! after every frame's turn. The shell's renderer reads it in place, converts
//! the projections to WebGPU's depth range, lays the numbers into its own
//! uniform buffers and draws; it decides nothing.

use crate::app::setting::store::use_settings_store;
use crate::global::Global;
use crate::module::map::environment::SKY_DOME_SCALE;
use crate::module::world::environment::config::rim_cfg;
use crate::module::world::frame::view::view;
use crate::module::world::lod::{fade_end_u, fade_start_u, LOD_CELL_PER_DISTANCE};
use xpute_core::kit::mat4::{mat4, mat4_mul};

const EDGE_STRENGTH: f64 = 0.3;
const EDGE_WIDTH: f64 = 0.03;

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
    pub const MSAA: usize = 87;
}

static STAGE: Global<[f64; STAGE_WORDS]> = Global::new();

fn stage() -> &'static mut [f64; STAGE_WORDS] {
    STAGE.get(|| [0.0; STAGE_WORDS])
}

/// The lane, for the renderer to lay the frame's uniforms out from.
pub fn stage_lane() -> &'static [f64; STAGE_WORDS] {
    stage()
}

/// Writes the frame's uniforms from the view and the settings as they stand.
pub fn stage_write() {
    let v = view();
    let settings = use_settings_store();
    let s = stage();
    let mut m = mat4();
    mat4_mul(&mut m, &v.view_matrix, &v.root);
    s[STAGE_AT::BADGE_VIEW..STAGE_AT::BADGE_VIEW + 16].copy_from_slice(&m);
    s[STAGE_AT::PROJ..STAGE_AT::PROJ + 16].copy_from_slice(&v.proj);
    s[STAGE_AT::LOCAL_VIEW_PROJ..STAGE_AT::LOCAL_VIEW_PROJ + 16].copy_from_slice(&v.local_view_proj);
    s[STAGE_AT::VIEW_MATRIX..STAGE_AT::VIEW_MATRIX + 16].copy_from_slice(&v.view_matrix);
    s[STAGE_AT::EYE_LOCAL..STAGE_AT::EYE_LOCAL + 3].copy_from_slice(&[v.eye_local.x, v.eye_local.y, v.eye_local.z]);
    s[STAGE_AT::EYE..STAGE_AT::EYE + 3].copy_from_slice(&[v.eye.x, v.eye.y, v.eye.z]);
    s[STAGE_AT::TRANSLATION..STAGE_AT::TRANSLATION + 3].copy_from_slice(&[v.tx, v.ty, v.tz]);
    s[STAGE_AT::VIEWPORT] = v.width;
    s[STAGE_AT::VIEWPORT + 1] = v.height;
    s[STAGE_AT::FOV] = v.fov;
    s[STAGE_AT::NEAR] = v.near;
    s[STAGE_AT::FAR] = v.far;
    s[STAGE_AT::FADE_START] = fade_start_u();
    s[STAGE_AT::FADE_END] = fade_end_u();
    s[STAGE_AT::LOD_CELL] = LOD_CELL_PER_DISTANCE();
    s[STAGE_AT::EDGE_STRENGTH] = if settings.show_edge_shade { EDGE_STRENGTH } else { 0.0 };
    s[STAGE_AT::LOW_SPEC] = if settings.texture_pack { 0.0 } else { 1.0 };
    s[STAGE_AT::EDGE_WIDTH] = EDGE_WIDTH;
    s[STAGE_AT::RIM_INTENSITY] = if settings.show_rim_light { rim_cfg::INTENSITY } else { 0.0 };
    s[STAGE_AT::SHADOWS] = if settings.show_shadow_map { 1.0 } else { 0.0 };
    s[STAGE_AT::SKY_RADIUS] = SKY_DOME_SCALE;
    // MSAA is the renderer's targets and every pipeline's sample count: a
    // change is a new renderer over the same canvas.
    s[STAGE_AT::MSAA] = if settings.msaa { 1.0 } else { 0.0 };
}
