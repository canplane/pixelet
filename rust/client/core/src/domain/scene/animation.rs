// pxl-core/domain/scene/animation.rs

//! Whether an animation of the scene stepped this frame. An animation (a
//! character, the clouds, the location indicator) wakes the frame itself at
//! each of its steps. The renderer reads this flag (pxl-render
//! `FrameInput::animated`) so that a frame drawn for an animation's step asks
//! for no frames after it: the animation's next step wakes the frame again.

use pxl_global::Global;

#[derive(Default)]
struct Animation {
    animated: bool,
}

static ANIMATION: Global<Animation> = Global::new();

fn animation() -> &'static mut Animation {
    ANIMATION.get(Animation::default)
}

/// Records that an animation stepped this frame. Only an animation that wakes
/// the frame at its own next step calls this, since the renderer then asks for
/// no frame after this one.
pub fn note_animation() {
    animation().animated = true;
}

/// Whether an animation stepped this frame. Reading it clears it, so the stage
/// reads it once a frame.
pub fn take() -> bool {
    core::mem::take(&mut animation().animated)
}
