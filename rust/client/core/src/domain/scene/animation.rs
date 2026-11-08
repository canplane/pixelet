// pxl-core/domain/scene/animation.rs

//! Whether an animation of the scene stepped this turn. An animation (a
//! character, the clouds, the location indicator) wakes the turn itself at
//! each of its steps. The renderer reads this flag (pxl-render
//! `FrameInput::animated`) so that a frame drawn for an animation's step asks
//! for no frames after it: the animation's next step wakes the turn again.

use xpute_core::global::Global;

#[derive(Default)]
struct Animation {
    animated: bool,
}

static ANIMATION: Global<Animation> = Global::new();

fn animation() -> &'static mut Animation {
    ANIMATION.get(Animation::default)
}

/// Records that an animation stepped this turn. Only an animation that wakes
/// the turn at its own next step calls this, since the renderer then asks for
/// no frame after this one.
pub fn note_animation() {
    animation().animated = true;
}

/// Whether an animation stepped this turn. Reading it clears it, so the stage
/// reads it once a turn.
pub fn take() -> bool {
    core::mem::take(&mut animation().animated)
}
