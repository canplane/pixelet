// pxl-render/frame/stage.rs

//! The frame's uniforms as the kernel decides them, in one lane of doubles at a
//! fixed address; the renderer reads it in place.

use xpute_guest::global::Global;

pub const STAGE_WORDS: usize = STAGE_AT::SHADOWS + 1;

/// Where each value sits in the lane.
#[expect(
    non_snake_case,
    reason = "the lane's offsets read as consts, `STAGE_AT::SHADOWS`"
)]
pub mod STAGE_AT {
    /// The view matrix through the root frame (16).
    pub const MARKER_VIEW: usize = 0;
    /// Camera to the device's clip, as `mat4::perspective` makes it (16).
    pub const PROJ: usize = 16;
    /// Eye-relative, so the matrix holds no eye translation for a point's to cancel against (16).
    pub const EYE_VIEW_PROJ: usize = 32;
    /// The eye, in the world's doubles (3).
    pub const EYE: usize = 48;
    /// The viewport's width in CSS pixels.
    pub const VIEWPORT_WIDTH: usize = EYE + 3;
    /// The CSS pixels a unit spans a unit from the eye, as the kernel's screen
    /// counts them (pxl-kernel domain/view/frame/screen.rs `px_per_unit`):
    /// what the markers are sized by, as the kernel lays them out.
    pub const PX_PER_UNIT: usize = VIEWPORT_WIDTH + 1;
    pub const CELL: usize = PX_PER_UNIT + 1;
    pub const EDGE_STRENGTH: usize = CELL + 1;
    pub const LOW_SPEC: usize = EDGE_STRENGTH + 1;
    pub const EDGE_WIDTH: usize = LOW_SPEC + 1;
    pub const RIM_INTENSITY: usize = EDGE_WIDTH + 1;
    pub const SCREEN_AO: usize = RIM_INTENSITY + 1;
    /// How much the inside of a cube's corners darkens, none where off.
    pub const CORNER_AO: usize = SCREEN_AO + 1;
    /// How much of the clouds shows, none to all.
    pub const CLOUDS: usize = CORNER_AO + 1;
    /// How far from the eye the sun's shadows are cast, in units: as far as
    /// the camera grid asks the ground's leaf cells or finer, where the
    /// casters are drawn whole; none where the shadows are off.
    pub const SHADOWS: usize = CLOUDS + 1;
}

static STAGE: Global<[f64; STAGE_WORDS]> = Global::new();

/// The lane: the kernel writes it a turn, the renderer reads it a frame.
pub fn stage_lane() -> &'static mut [f64; STAGE_WORDS] {
    STAGE.get(|| [0.0; STAGE_WORDS])
}
