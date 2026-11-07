// pxl-kernel/domain/scene/animation.rs

//! Whether anything the scene animates stepped this turn, waking the turn at
//! its own steps (`animated`), which the renderer reads so the resolve asks
//! no frame after it (pxl-render `FrameInput::animated`).

use xpute_core::global::Global;

#[derive(Default)]
struct Animation {
    animated: bool,
}

static ANIMATION: Global<Animation> = Global::new();

fn animation() -> &'static mut Animation {
    ANIMATION.get(Animation::default)
}

/// What the scene animates stepped this turn: what wakes the turn at its
/// own steps, so no frame is asked after it.
pub fn note_animation() {
    animation().animated = true;
}

/// Whether what is animated stepped; the turn's taken.
pub fn take() -> bool {
    core::mem::take(&mut animation().animated)
}
